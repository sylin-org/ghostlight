# 0012. UI parity + input fidelity (phantom cursor, virtual key codes)

- Status: Accepted
- Date: 2026-07

## Context
A person may be watching their own browser while the agent drives it, so the extension
must show what the agent is doing: part of the North Star "watching" delight and of
true parity with the official Claude-in-Chrome, which renders on-page affordances.
Separately, input must actually take effect: dispatching `keyDown`/`keyUp` without a
Windows virtual key code delivers modified combos that Chrome maps to no editing
command, so `ctrl+a` is a no-op and a following `type` appends instead of replacing; and
a synthetic `ctrl/cmd+r` or `F5` never reloads the tab.

## Decision
(a) UI affordances. A dedicated content script `extension/agent-visual-indicator.js`
(all_urls, document_idle) renders a phantom cursor (its own SVG arrow, tip at the
target, Claude-orange with a white outline and glow) and a subtle "agent active" glow
border. The service worker sends `UPDATE_PHANTOM_CURSOR` with the rescaled CSS-px
coordinate before every mouse dispatch (click/hover/drag endpoints/scroll) and awaits
the settle, so the user sees the pointer arrive before the action fires;
`SHOW_AGENT_INDICATORS` raises the glow at the start of every `computer` action
(self-fades ~4s, respects `prefers-reduced-motion`). Before capture, `HIDE_FOR_TOOL_USE`
(+40ms) then `SHOW_AFTER_TOOL_USE` keep the overlay out of the model's screenshot; the
overlay's `browser-mcp-*` elements are excluded from `read_page`/`find`. The official's
Stop button and static chat pill are omitted: they are product controls for the
official's in-browser agent (the official suppresses Stop in isMcp mode) and do not
apply to our external-client model.

(b) Input fidelity. `vkCode()` supplies Windows virtual key codes for A-Z, 0-9, and
named keys; `pressKey` sends `windowsVirtualKeyCode`/`nativeVirtualKeyCode` on
keyDown/keyUp so editing shortcuts (ctrl+a select-all, ctrl+c/x, arrows) register as
real commands. Reload chords (ctrl/cmd+r, F5) are intercepted and driven via
`chrome.tabs.reload` (bypassCache when shift is held).

Both are reimplemented from the official technique, not copied. (Commits 117bbdb, 1135866.)

## Consequences
Positive: the watching user sees the cursor arrive and a running-activity glow; editing
shortcuts work (focus -> ctrl+a -> type now replaces the field). Because two content
scripts now share `sendMessage`, content.js returns false for messages that are not its
own so both do not answer the same request.

Negative / trade-offs: two content scripts run on every page instead of one, and the
pre-dispatch cursor-settle wait adds a small latency to each mouse action. Reimplementing
rather than copying avoids coupling to upstream code at the cost of tracking the
official's behavior by observation.
