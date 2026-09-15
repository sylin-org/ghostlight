# ADR-0169: Extension pipeline and resource hygiene

Date: 2026-09-14. Status: Accepted by owner direction.

Builds on ADR-0005, ADR-0045, ADR-0078, ADR-0138, and ADR-0168.

## Context

A comprehensive code and architecture audit of the Ghostlight Chrome Extension (Manifest V3)
identified latency traps, redundant asynchronous roundtrips, memory retention risks, and
file concentration that accumulated during rapid 1.0 feature development:

1. **Form Fill Artificial Latency**: `content.js` enforced `FILL_DOCUMENT_MIN_AGE_MS = 6000`,
   forcing every form fill to sleep until 6 seconds after page navigation. This was added as a
   heuristic during GenAI.Works investigation, but Research 29 and ADR-0168 proved the reset was
   caused by site-side React Hook Form visibility/focus handlers and cured by Chromium CDP focus
   emulation. The 6-second delay remained as dead latency penalty on all form fills.
2. **Input Verification Amplification**: In `service-worker.js` and `lib/documents.js`, low-level
   CDP mouse and key events (`Input.dispatchMouseEvent`, `Input.dispatchKeyEvent`) called
   `documents.input()` on every sub-packet. A single click triggered 3 document frame queries and
   content-script roundtrips; a drag operation triggered 14 or more. Verification belongs once
   per composite operation, not on raw CDP packets.
3. **Detached Element Retention**: `content.js` cached DOM elements in a strong `Map`
   (`locators`). Elements replaced or detached by single-page application (SPA) frameworks
   stayed pinned in memory indefinitely.
4. **Redundant Pre-dispatch Session Storage**: `lib/engine.js` executed two consecutive storage
   writes (`{ phase: "accepted" }` followed immediately by `{ phase: "dispatched" }`) prior to
   running an operation, doubling storage IPC overhead on every command.
5. **Alarms Minimum Delay Clamp**: `chrome.alarms` enforces a 1-minute minimum delay in packaged
   extensions. The 0.05-minute (3-second) reconnect alarm fell back to 60 seconds when the worker
   remained alive.
6. **Obsolete Web APIs and Artifacts**: Obsolete no-op calls such as `range.detach?.()` and
   unused variable suppressions remained in content scripts.

## Decision

1. **Remove Form Fill Navigation Delay**: Remove `FILL_DOCUMENT_MIN_AGE_MS` and the 6-second
   sleep from `prepareStableFill` in `content.js`. Document readiness continues to require
   complete or interactive readyState and visible body layout without artificial sleep.
2. **Composite Action Input Verification**: Verify document authority and frame context once
   at the start of composite browser actions (`click`, `drag`, `type`, `activate`). Raw CDP
   dispatch packets execute without repeating document frame roundtrips.
3. **Prune Detached Locators**: In `content.js`, inspect and prune disconnected nodes
   (`!element.isConnected`) periodically or when cache thresholds are reached, preventing DOM
   leaks during SPA re-renders.
4. **Single-step Dispatch Persistence**: In `lib/engine.js`, record operation transition directly
   to `dispatched` before command execution, eliminating the redundant `accepted` storage write.
5. **Hybrid Reconnect Loop**: Use in-memory timers (`setTimeout`) for rapid 3-second reconnect
   retries while the service worker is running, backed by `chrome.alarms` for cold worker wakeup.
6. **API and Dead Code Cleanup**: Remove deprecated DOM API usages (`range.detach`) and clean
   redundant variable suppression patterns across extension scripts.
7. **Preserve Policy-Free Extension Seams**: All hygiene improvements remain strictly mechanical
   and policy-free. No governance, permission checking, or tool schemas move into the extension.

## Consequences

- Form fill operations execute immediately upon DOM interactive readiness, eliminating up to 6
  seconds of artificial latency.
- Mouse clicks, typing, and drag interactions no longer generate redundant frame queries for
  every low-level CDP event.
- Single-page application memory consumption is bounded as unmounted DOM nodes are freed.
- Pre-dispatch storage latency is halved per operation.
- Reconnection latency is 3 seconds in active workers rather than waiting for Chrome's 60-second
  alarm clamp.
- All existing 267 extension tests, adapter protocol major 2 contracts, and orchestrator
  integration boundaries remain green and compatible.
