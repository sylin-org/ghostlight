# Break 2.0: extension lane ledger

Lane: extension. It owns new files under `extension/`, `tests/golden/protocol4/**`, the level-4
page assets under `crates/orchestrator/src/browser/adapter/chromium/level4/assets/`,
`scripts/lint-js*.mjs`, and this ledger. Ground rules are in `BOOTSTRAP.md` when the connectors
lane has landed it. The target is `docs/adr/0197-contained-seams-and-the-2-0-break.md`; the wire
is `docs/design/adapter-protocol-4.md`.

BASE: `f28bcbd7` (dev when the lane started).

Gate: `npm test --prefix extension`, `node --check` on every new or changed `.js`, and from
extension-2 on `node --test "crates/orchestrator/src/browser/adapter/chromium/level4/assets/tests/*.test.js"`.
The lane builds no Rust.

## RESUME HERE

Next slice: `extension-9-js-lint`, the one after `extension-8-p4-worker` in `lanes-break.json`
(deferrable; it must land before the 2.0 release, not before the deploy). The worker entry is
done: `extension/worker.js` speaks protocol 4 and is loaded by nothing until switch-3 points the
manifest at it. The level-4 page assets are complete for this lane: `content.js` dispatches
exactly the 32 vocabulary kinds, every throw site is an `AssetError` with its code, and the
cancel token ends `observe`, `prepare_fill`, `verify_fill_retained` and the sensor waits. Any
change to the six sources changes the bundle hash, so update `tests/golden/protocol4/assets.sha256`
deliberately in the same commit (print it with `bundle.sha256(bundle.expand())` from
`level4/assets/tests/bundle.js`).

## Landed

| Slice | What | Commit |
|---|---|---|
| extension-8-p4-worker | `extension/worker.js`, the protocol-4 entry (not yet the manifest's worker). The hello is the first frame (`attended` and the engine version are read before `connectNative`), single flight; no `hello_accepted` in 10 s sets the `service_outdated` link state with "Update Ghostlight on this computer."; the router per spec 2.2 (heartbeat, `hello_accepted`, `command_chunk`, `request`, `cancel`, `notice`, `control_state`, `backend_unavailable`, `error`; unknown kinds ignored); `hello_accepted` checks and registers the assets, settles state only on a new epoch, and a request waits for that work; the controlled-tab set (managed or released, in `storage.session`, restored and re-retained on worker start) with events for controlled tabs only and a released tab reporting one commit; the chrome listener table; AMBIENT on tab complete, control change, preference change and port loss; a closed port cancels in-flight primitives and reconnects at once after `backend_unavailable`, else after 3 s through the alarm; `onInstalled`, `onStartup`, `toggle-hold`; the UI messages (`ui_snapshot`, `get_preferences`, `set_preferences`, `runtime_control`, `release_debugger_sessions`, `connection_diagnostics`; `diagnostics_toggle` answers an error). `lib/chunks.js` gains the `correlationOf` option (default: the protocol 3 nesting) because a protocol 4 request carries its correlation at the top. Tests: `worker.test.js` (38, worker loaded into `node:vm`), one more in `chunks.test.js`. | see `git log` |
| extension-7-p4-buffers-dispatch | `lib/arguments.js` (`GhostlightArguments`): the argument checks the primitives share (ids, bounded text, whole numbers, document guard, frame reference, world call). `lib/evaluate.js` (`GhostlightEvaluate`): `evaluate_main` over the unchanged `script-evaluator.js`; the debugger attaches only after the script parses, the guard and the dialog are checked right before the send, the deadline calls `Runtime.terminateExecution` and answers `timeout` with `dispatched` true, a page exception is a result. `lib/buffers.js` (`GhostlightBuffers`): `diagnostics` read (binding reset, guard) and clear; `recording` start with a watch, status, stop, discard and export to download, client or page (the page export runs the world call with the GIF as its attachment); the offscreen encoder as the old worker used it; recording namespace `protocol4`; the event hooks the worker feeds. `lib/primitives.js` (`GhostlightPrimitives`): the 17-entry table, `run` with exactly one receipt, `unsupported` for unknown names, `invalid_request` for validator failures, the cancel registry (60 s), per-primitive effect watches in a per-tab set, the `failureCode` table, `dialog` and `accessibility_snapshot`. `capture.js` now exports `decodeImageSize`. Tests: `evaluate-main.test.js` (14), `buffers.test.js` (39), `primitives.test.js` (42). | see `git log` |
| extension-6-p4-tabs-capture-notice | `lib/tabs.js` (`GhostlightTabs`): `listTabs` with `opener_tab_id`, `openTab` (group by key from `storage.session`, title fallback after reload, duplicate merge, `focus_window`, adopt, ready wait with timeout and cancel), `navigate` (url, back, forward, reload, `discard_beforeunload` with `acceptBeforeUnload`), `focusTab`, `closeTab` (preserve-tabs veto, released marking), `setZoom`, `resizeWindow`. `lib/capture.js` (`GhostlightCapture`): `view` and `capture` (clip kinds, budget scale, quality ladder, `capture_too_large`, measured image size, hide through the world's per-tab hold count inside `finally`). `lib/notice.js` (`GhostlightNotice`): `deliver`, `flush`, `flushWindow`, `activityList`, deferral through `presentation-queue.js` under one namespace, quiet completion, never attaches. Each primitive handler is `{validate, run, addresses, tabOf}`. Tests: `tabs.test.js` (35), `capture.test.js` (20), `notice.test.js` (15), the scripted browser and golden driver in `tests/p4-chrome.js`. | see `git log` |
| extension-5-p4-world-inventory-input | `lib/world.js` (`GhostlightWorld`): fixed SHIMS and WRAP, `setAssets` with the sha256 check, `register` as the sessions installer, objectId world handles with a loader re-check, `owner_of` through `DOM.getFrameOwner`, `callOne` with the dialog and deadline race and one lazy reinstall, `mapCallError`, `callPrimitive` (sequence, parallel, race), cancel tokens, `ambientAll`, `present`, `disposeTab`, `devicePixelRatio`, `documentObject`, `localFacts` with capture holds, and the dialog watch. `lib/inventory.js` (`GhostlightInventory`): `snapshot` over root and child sessions, unseen urls, `topSnapshot`, `checkGuard`, `currentLoader`, `framesPrimitive`. `lib/input.js` (`GhostlightInput`): the input batch with precondition, dialog race and drag interception (`dragIntercepted`, `cancelDragInterception`). Tests: `world.test.js` (22), `inventory.test.js` (11), `input.test.js` (13), shared doubles in `tests/p4-fakes.js`. | see `git log` |
| extension-4-p4-sessions | `lib/sessions.js` (`GhostlightSessions`): attach leases, retained tabs, focus emulation, dialogs as `{type, message, sessionId}`, opt-in domains, `CDP_ALLOW` (33 methods) and `cdp()` with `browser_refused`, root and child session registry with registration identifiers, `forEachSession`, `rootSession`, `childSessions`, `onRelease` and `onSessionGone`; `tests/sessions.test.js` (41 tests). `lib/debugger.js` is untouched until the switch deletes it. | see `git log` |
| extension-3-p4-asset-content | Level-4 `content.js` behaviour: coded errors, locator epochs, cancel tokens, `verify_fill_retained`, `observe` `until_satisfied`, `hit_point`, `owner_box`, `owner_probe`, `focus_probe`, `require_focused`, `focus_text_field`, `attach_file`; `tests/content.test.js` (85 tests); new asset hash. | see `git log` |
| extension-2-p4-asset-shell | `level4/assets/` shell (world, page-shared, sensor, presentation-css, presentation, content), `tests/bundle.js`, `tests/fake-dom.js`, `tests/world.test.js` (18 tests), `tests/golden/protocol4/assets.sha256`. | see `git log` |
| extension-1-p4-contracts | `tests/golden/protocol4/` (vocabulary, 10 frames, 6 events, 40 requests, 80 receipts, 32 world messages, 12 tolerance cases), `extension/lib/protocol.js`, `extension/tests/golden-protocol4.test.js`. | see `git log` |

## Counts

- 17 primitives, 13 receipt failure codes, 10 call failure codes, 32 world message kinds,
  22 asset error codes (19 pre-effect).
- 22 tests in `golden-protocol4.test.js`, 41 in `sessions.test.js`, 22 in `world.test.js`, 11 in
  `inventory.test.js`, 13 in `input.test.js`, 35 in `tabs.test.js`, 20 in `capture.test.js`, 15 in
  `notice.test.js`.
- 38 tests in `worker.test.js`.
- 19 tests in `level4/assets/tests/world.test.js`, 85 in `level4/assets/tests/content.test.js`.

## Fixture conventions

- Every wire fixture is a whole frame, so `requests/*` and `receipts/*` carry `kind`,
  `correlation`, and the primitive or outcome.
- Request files are `<primitive>[.<variant>].json`. Receipt files are `<primitive>.<case>.json`,
  where the case is `ok`, `ok_<name>` for another successful outcome, or the failure code. The
  one receipt for an unknown primitive is `unknown_primitive.unsupported.json`.
- An absent optional field is left out of the JSON, as the spec's receipt examples do. A field
  with a serde default that is not an `Option` (mouse `button`, `buttons`, `click_count`,
  `modifiers`, `delta_x`, `delta_y`, `force`) is always written. The service types must match
  this when switch-2 binds them to the fixtures.
- World messages are the exact messages in `world/<kind>.json`. A request that carries a world
  call must use that fixture message unchanged; the test enforces it. `scroll` shows all three
  optional fields at once so the fixture lists every field.
- `ok_*` recording cases cover each `RecordingFacts` result. The nested `delivery` object in an
  export outcome is the tagged `Delivery` value, `{"delivery": "attached", "tab_id": 12}`.
- Tolerance files are `{description, frame, expect}`. `expect` names one form:
  `decodes_as` (the frame holds everything in the named fixture plus extra fields, and decodes
  to that fixture), `ignored` (an unknown frame kind, event or intent that both sides drop),
  `maps_to` (`other` or `unknown`), or `answered` (the extension replies with the named
  receipt).
- The `error` frame is listed only under service-to-extension, as the spec's frame table has
  it, although the spec's Rust `Inbound` enum also decodes one.

## Notes

- `protocol.js` carries the vocabulary as code and never reads the JSON; the test compares the
  two. It adds `isKnown(list, name)` and upper-case aliases for the scalars the spec's
  pseudo-JS uses (`WORLD_NAME`, `NATIVE_HOST_NAME`, `MAX_UNSEEN`, and so on).
- `lib/protocol.js` is not yet loaded by any worker. `service-worker.js` still runs the
  protocol 3 path until the switch deletes it.
- The worker wiring for `world.js`: create the sessions with `installer: (s) => world.register(s)`,
  `onRelease: (t) => world.disposeTab(t)` and `onSessionGone: (t, s) => world.sessionGone(t, s)`,
  then the inventory, then the world (see `tests/p4-fakes.js` `wire`). The worker calls
  `world.announceDialog` on `Page.javascriptDialogOpening`, `world.dropTab` on `debugger.onDetach`,
  `input.dragIntercepted` on `Input.dragIntercepted` and `input.cancelDragInterception` on a
  frame-0 commit. Handles survive between primitives only on retained tabs: a lease release that
  detaches runs `disposeTab`.
- `lib/sessions.js` is not yet loaded by any worker. `create(debuggerApi, {installer, onRelease,
  onSessionGone, protocolVersion})` requires the installer. A session is
  `{target, targetId, identifiers}` with a live `identifiers` list. The installer runs once per
  attachment for the root and once per child; the root also gets the iframe `Target.setAutoAttach`
  after its installer, which is what makes child sessions arrive. A child that ended before its
  setup finished does not fail the tab.
- Worker wiring for the extension-7 modules (load order: `arguments.js` after `protocol.js` and
  `sessions.js`; `buffers.js` and `evaluate.js` before `primitives.js`; `arguments.js` is not in the
  extension-8 import list of the spec, add it):
  - Create `cancellations = primitives.createCancellations()` first and pass its `isCancelled` to
    `tabs.create`, `world.create` and `input.create`.
  - Create the recording store with `onStop: (...stop) => buffers.recordingEnded(...stop)` and the
    diagnostics store, then `buffers.create({lifecycle, inventory, worldCalls: world, diagnostics,
    recording, changed})`. `changed()` refreshes the popup and badge after a recording starts or ends.
    The world's `local.recording(tabId)` is `Boolean(recording.activeForTab(tabId))`.
  - `evaluate.create({lifecycle, inventory})`, then `primitives.create({send, controlled, cancellations,
    lifecycle, inventory, world, tabs, capture, input, buffers, evaluate})`. `send(frame)` writes to the
    native port.
  - Frames: `request` -> `primitives.run(frame.correlation, frame.primitive)`; `cancel` ->
    `primitives.cancel(correlation)`; a rejected chunk transfer -> `primitives.reject(correlation,
    "invalid_request", reason)`; port loss -> `primitives.disconnect()` (cancels in-flight work and
    drops its receipts).
  - `webNavigation.onCommitted`: `primitives.committed(tabId, url)` for frame 0, and
    `buffers.committed(details)` for every frame. `onBeforeNavigate`: `buffers.beforeNavigate(tabId)`.
    `debugger.onEvent` `Page.screencastFrame` with no `sessionId` in the source:
    `buffers.screencastFrame(tabId, params)`. `debugger.onDetach`: `buffers.tabDetached(tabId)`. A new
    service epoch: `await buffers.settleBoundaryState()`.
  - `controlled.manage(tabId)` runs before every addressing primitive (it must retain the debugger).
    A recording survives its lease only on a retained tab.
  - A failed `acquire` surfaces as the browser's own error text; `failureCode` maps it by message to
    `debugger_unavailable`, `tab_missing` or `session_lost`, and a CDP error (JSON text with a
    numeric code) to `browser_refused`.
- Worker wiring for the extension-6 modules: `tabs.create({lifecycle, controlled, isCancelled})` needs
  the worker's controlled-tab hook `{manage(tabId), release(tabId), managedIds()}`. The worker calls
  `tabs.acceptBeforeUnload(tabId, dialog)` on every `Page.javascriptDialogOpening`.
  `capture.create({lifecycle, worldCalls})` uses the world's hold count for `hidden`.
  `notice.create({worldCalls, inventory, queue, changed})` takes the presentation queue instance;
  the worker persists the queue and refreshes the popup from `changed()`, calls `flush(tabId)` on
  tab activation and tab complete and `flushWindow(windowId)` on `windows.onFocusChanged`, and reads
  `activityList()` for the popup. `open_tab` marks its own tab controlled, so its handler
  `addresses` is false; `close_tab` is also not addressing.
- Worker notes (extension-8):
  - The worker imports `lib/arguments.js` too (the spec list omits it; `primitives.js`, `buffers.js`
    and `evaluate.js` read `GhostlightArguments`).
  - The manifest, `shared.js` and `state.js` are untouched. The `service_outdated` link state is
    computed in the worker's snapshot; switch-3 moves it into `shared.linkState` and drops
    `attention` from `state.js`. `popup.js` and `options.js` still read `compatible` and `link_state`
    as before and show "Version mismatch" for it until switch-3 edits them.
  - `chunks.js` keeps its protocol 3 default for `correlationOf` only for the legacy worker; switch-3
    deletes the default and the nested-shape tests with `service-worker.js`.
  - The hello timer starts at the hello and ends at `hello_accepted`, `backend_unavailable` or a closed
    port, so a service that closes the socket on the hello shows "unreachable", not "outdated".
  - `release_debugger_sessions` and a held or ended control state interrupt recordings (`browser_detached`
    and `runtime_held`) and drop diagnostics; a new epoch or a closed port goes through
    `buffers.settleBoundaryState()` (`service_disconnected`).
