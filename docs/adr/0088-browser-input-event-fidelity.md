# ADR-0088: Browser input event fidelity

Status: Accepted

Date: 2026-07-20

## Context

Live Ghostlight testing found a split between visible feedback and browser behavior. A printable
`computer.key` call produced a trusted `keydown` and the correct privacy-safe key cue, but the
focused field did not receive the character. Function keys and standalone modifiers arrived with
incomplete identity. Pointer click and wheel paths worked, while the drag path emitted pointer and
mouse movement without starting native HTML drag and drop. Two result paths could also overstate
what happened: `scroll_to` accepted no target, and coordinate image placement claimed a drop even
when the page did not signal handling.

These were not separate product problems. They were one boundary-design problem: CDP packet
construction was repeated inside orchestration code, so event fields and acknowledgement language
could drift by action.

## Decision

### D1. Pure input domains own complete browser packets

`extension/lib/keys.js` owns keyboard dispatch plans. `extension/lib/input-events.js` owns pointer
dispatch descriptors. The service worker owns orchestration only: target resolution, timing,
dispatch order, observation, and presentation.

The domains are pure and covered with exact descriptor tests. They contain no Chrome calls,
presentation state, page content, or governance logic.

### D2. Printable key calls carry insertion text

An unmodified printable `computer.key` keydown carries CDP `text` and `unmodifiedText` in addition
to `key`, `code`, modifiers, and virtual key codes. This makes the call both a real keyboard event
and an editing action in a focused ordinary or protected field. Command shortcuts using Ctrl,
Alt, or Meta omit text so the browser interprets them as commands rather than literal insertion.

Uppercase and shifted US-QWERTY punctuation use the base physical code, the shifted visible key,
and the Shift modifier. F1 through F24 carry their Windows virtual key codes. A standalone
modifier carries its left-side DOM code and virtual key code; its keyup packet clears that
modifier's state.

Reload shortcuts remain direct `chrome.tabs.reload` operations because renderer-delivered
synthetic keys do not perform browser chrome commands.

This amends ADR-0087 D4's dispatch-unchanged detail. ADR-0087 still controls disclosure: a
protected field receives the character, but its visual cue remains an unlabeled keycap and no key
identity enters the page presentation message.

### D3. Text typing is planned once and counted as the user sees it

`computer.type` uses the same keyboard domain. ASCII keyboard characters receive complete
keydown/keyup pairs. Characters without a physical US-QWERTY mapping use `Input.insertText`, which
preserves Unicode and input-method-style insertion. A CRLF pair becomes one Enter action.

The result counts Unicode code points after CRLF normalization. It no longer reports JavaScript
UTF-16 code units as characters.

### D4. Pointer state is explicit for every packet

Every mouse move, press, release, and wheel event is produced by the pointer domain. Idle moves and
wheels explicitly carry no button and zero force. Held-button drag moves carry the left-button
identity, left-button bit, and nonzero force. Press and release packets carry a click count of at
least one. Chromium uses both the active button identity and the button bitmask when deciding
whether a move can begin native drag.

The drag path uses click count one on its press and release. This supersedes the no-click-count
implementation detail in task T09. Click count is packet hygiene. It does not, by itself, turn a
held pointer gesture into native HTML drag and drop.

### D5. Drag execution has pointer and native lanes

`left_click_drag` represents one user intent but two browser mechanisms:

- Pointer lane: sliders, text selection, canvas interactions, and application-defined pointer or
  mouse handlers receive the ordinary move, press, held-button movement, and release sequence.
- Native lane: HTML draggable content uses an action-scoped `Input.setInterceptDrags` session. CDP's
  `Input.dragIntercepted` payload is replayed through `Input.dispatchDragEvent` as dragEnter,
  dragOver, and drop before the mouse is released.

A per-tab drag coordinator lives under the existing per-tab command FIFO. Its phases are armed,
native, pointer, and cancelled. A trusted content-side dragstart observer reports only whether the
event occurred and whether the page cancelled it. It never retains the event target, dragged text,
`DataTransfer`, or page content. A non-cancelled native dragstart receives the bounded native wait;
an ordinary pointer gesture receives only a short event-order grace period.

CDP `DragData` remains an opaque object inside the extension service worker. It is never decoded,
logged, sent to the native host, included in audit data, passed to presentation, or returned in a
tool result. Interception is enabled only for the active action and is disabled before replay.
Error, navigation, tab close, debugger detach, and panic paths cancel the rendezvous and retire
interception, native drag state, and any held mouse button. Unsupported interception falls back to
the pointer lane.

Synthetic screenshot placement remains a separate DOM mechanism. It does not enter the native
drag coordinator and does not claim equivalence with an operating-system file drag.

### D6. Input acknowledgements describe evidence, not intent

`scroll_to` requires a ref or coordinate before entering its observation path. Coordinate image
placement still dispatches the standard dragenter, dragover, and drop sequence, but reports only
that dispatch unless cancellation shows that the page signaled handling. It does not claim that an
application accepted or saved the image.

Semantic form and file setters remain distinct mechanisms. Setting a value or `FileList` followed
by input/change events is correct for those APIs and is not expected to synthesize keyboard or
native operating-system events.

## Consequences

- Ordinary and protected fields both receive printable `computer.key` characters, while ADR-0087
  keeps protected presentation private.
- Function keys, standalone modifiers, shortcuts, clicks, hovers, wheels, and drags share one
  tested packet vocabulary.
- Pointer-only drags avoid the full native-drag wait. Native HTML drags use the browser's opaque
  drag payload and lifecycle instead of fabricated DOM events.
- Drag interception cannot remain enabled as page-wide ambient state after an action completes.
- Unicode typing receipts match user-perceived code points rather than UTF-16 storage units.
- Callers can distinguish a dispatched coordinate drop from page-signaled handling.
- Browser-vendor adapters can later translate these semantic plans without moving policy or
  orchestration into the extension.

## Verification

The automated matrix covers exact packets for ordinary and shifted printables, command shortcuts,
function keys, standalone modifiers, reloads, Unicode insertion, CRLF normalization, idle and
held pointer moves, click button state, wheel state, architectural wiring, and false-success
guards.

The live Ghostlight matrix must cover:

- printable key insertion into ordinary and password fields, with distinct visible cues;
- uppercase and shifted punctuation insertion;
- F2 virtual key identity and standalone Shift state;
- native draggable dragstart, dragover, drop, and dragend;
- pointer-only slider or selection behavior without a native dragstart;
- Unicode type insertion and receipt count;
- representative click, hover, shortcut, and scroll regressions.

Live verification on 2026-07-20 passed both drag lanes in the ordinary Ghostlight stack. A native
HTML draggable preserved its page-authored payload and produced dragstart, dragenter, dragover,
drop, and dragend. A pointer-only range input moved from 10 to 84 through ten input events, retained
the held-button pointer sequence, and produced no additional native drag lifecycle. The earlier
failure established that Chromium requires held moves to carry both `button: "left"` and the left
button bitmask; either signal alone is incomplete.

## Rejected alternatives

- Patch only the protected-field case. Rejected because target privacy and event execution are
  separate concerns, and ordinary fields failed in the same way.
- Use `Input.insertText` for every key call. Rejected because it would erase keyboard event and
  shortcut semantics.
- Treat a complete mouse packet as sufficient for native HTML drag and drop. Rejected after live
  testing produced trusted pointerdown, mousedown, pointerup, and mouseup but no dragstart,
  dragover, drop, or dragend.
- Fabricate a `DataTransfer` and dispatch DOM drag events. Rejected because the platform gives
  native drag data a protected lifecycle, and synthetic dispatch can report success without
  producing application behavior.
- Leave drag interception enabled for the page. Rejected because it changes unrelated user and
  page behavior. The mechanism is scoped to one queued action and cleaned up on every terminal
  path.
- Claim success after dispatch. Rejected because dispatch is not evidence that an application
  accepted the action.

## Prior art

- Chrome DevTools Protocol Input domain: https://chromedevtools.github.io/devtools-protocol/tot/Input/
- Playwright Chromium drag coordinator technique: https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/chromium/crDragDrop.ts
- Puppeteer mouse drag and drop: https://pptr.dev/api/puppeteer.mouse.draganddrop
- Selenium Actions pointer sequence: https://www.selenium.dev/selenium/docs/api/javascript/Actions.html
- WHATWG HTML drag and drop processing model: https://html.spec.whatwg.org/multipage/dnd.html

## Related decisions

- ADR-0005: policy-free extension.
- ADR-0012: UI and input fidelity.
- ADR-0050: file tools and coordinate image placement.
- ADR-0078: closed-loop browser core and truthful receipts.
- ADR-0080: resource-scoped browser command scheduling.
- ADR-0087: privacy-safe key presentation.
