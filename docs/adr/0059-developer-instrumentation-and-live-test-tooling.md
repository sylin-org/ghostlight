# ADR-0059: Developer instrumentation and live-test tooling

## Status

Accepted.

## Context

Live-verifying ADR-0058 (per-browser identity, focus routing, composite tabId encoding) against a
real Chrome session took far longer than the code changes themselves, and the real bug it uncovered
(the `tabId` multiplier overflow -- see ADR-0058 section 4's live-verification note) was found by
manually correlating timestamps across a background-task log, `ghostlight doctor` output, and a raw
`debug-state-<pid>.json` file, none of which line up automatically. Three concrete gaps made this
slower and less certain than it needed to be:

1. **The native-host attach/reject decision is a `tracing::debug!` line, not a structured event.**
   `crates/core/src/hub/endpoint.rs` logs "dropped a stray connection" for both a genuine collision
   and a bare `doctor` probe (no hello at all) with the SAME text, and it goes to whichever
   background task's stdout happens to be running -- not into the same `debug-state-<pid>.json`
   file every other diagnostic already reads from.
2. **The browser-role relay is a total blind spot by default.** Chrome launches it with its own
   environment and never passes `--debug`; the only way to get instrumentation out of it is an
   inherited `GHOSTLIGHT_DEBUG` env var, which nothing in this session's tooling was setting.
3. **The extension has zero persisted logging.** Its only signal is live `console.log` output in a
   DevTools panel only the user can see, in the moment -- unusable for after-the-fact correlation,
   and it requires relaying findings back to the agent by hand.

Separately, verifying ANY of this against a real browser required repeatedly killing and rebuilding
processes shared with the user's other live Claude sessions (this session's own live-verification of
ADR-0058 required negotiating the shutdown of Claude Desktop and two unrelated VS Code windows just
to free a locked file), and hand-writing a throwaway test manifest that got its own schema wrong on
the first attempt. None of this is specific to ADR-0058; it will recur for every future feature that
needs a real Chrome round trip.

## Decision

### 1. Structured attach/reject events (service)

`Browser::attach`'s hello-read outcome (parsed OK + role + pid -> admitted/replaced, or the specific
reason it was NOT admitted: no hello sent at all, malformed JSON, or wrong role) becomes a
structured entry appended to the SAME debug-state "recent" ring buffer
(`ghostlight_transport::observability::DebugSink`) every other diagnostic already reads, instead of
a bare `tracing::debug!` line. A `doctor`-style bare probe (no hello) and a real relay's malformed
hello now read distinguishably in the log, instead of both saying "dropped a stray connection."

### 2. Browser-relay debug is opt-in via the ALREADY-DOCUMENTED env mechanism, wired into the new
   dev-browser launcher

No new gating logic: `ghostlight-relay`'s browser role already honors an inherited
`GHOSTLIGHT_DEBUG` (see `crates/relay/src/main.rs::run_browser`), the exact mechanism ADR-0046
originally documented and this session never actually used. The new `scripts/dev-browser` launcher
(Decision 5) sets it unconditionally when it starts Chrome, so every dev-loop browser session is
instrumented by construction -- no new code path, just actually using the one that exists.

### 3. Extension debug-event forwarding

A new, additive, fire-and-forget wire message (`crates/core/src/messages.rs` reference doc,
`{"type":"debug_event","event":"<name>","detail":{...}}`), sent by the extension ONLY when a
`chrome.storage.local` debug flag is on (default off, toggled from the options page, mirroring the
existing `ghostlight_captions` preference pattern). Covers the four moments this session's live
debugging actually needed and didn't have: `connect_attempt`, `connect_disconnect` (with
`chrome.runtime.lastError`'s message, if any), `hello_sent` is implicit (the relay already logs it
per Decision 2), and `focus_reported`. The service appends these into the SAME attaching browser's
debug-state recent ring (Decision 1's mechanism), so one file shows the extension's own view
interleaved with the service's, ordered by arrival.

### 4. An interactive fake-browser driver (`lightbox fake-browser`)

Extends the existing dev-only harness (ADR-0056) rather than adding a new binary: `lightbox` already
depends on `ghostlight-core`; this adds `ghostlight-transport` so `fake-browser` can dial a REAL
running service's extension endpoint exactly as `ghostlight-relay`'s browser role does (same
`endpoint_candidates`/`pick_native_host_endpoint`, same `ROLE_BROWSER` hello) without needing Chrome
at all. Prints every incoming frame as pretty JSON; `--auto-reply` answers every `tool_request` and
`tab_url_request` with a canned result -- DELIBERATELY using a billion-scale `tabId`
(`2_000_000_000 + a small counter`, comfortably past `i32::MAX`'s neighborhood) rather than a small
one, so a tabId-encoding bug like ADR-0058's is caught by the FIRST fake-browser round trip instead
of requiring a real Chrome session to surface it. Stdin commands (`focus`, `kill`, `reply <id>
<json>`) drive the rest by hand. This is the highest-leverage single addition: it turns "rebuild,
restart, reload the extension, wait, reload again" into "run one command," entirely offline.

### 5. `scripts/dev-browser`: an isolated, disposable Chrome profile

Launches Chrome with `--user-data-dir` pointed at a fresh directory under the repo's own
gitignored scratch area (never the user's real profile), loads the unpacked dev extension, and sets
`GHOSTLIGHT_DEBUG=1` in its environment (Decision 2). Safe to kill, reload, or inspect at any time --
it is provably not the user's real browsing session, and nothing else is ever attached to it, so no
future live-verification needs to negotiate with the user's other open windows/tools the way this
session did.

### 6. `scripts/dev-loop`: the manual dance, in one command

Kill stray dev-instance processes this repo's own target dir owns (never anything outside it, never
a bare `taskkill` by name), rebuild `ghostlight` + `ghostlight-relay`, restart the dev service with
the committed fixture manifest (Decision 7), poll `ghostlight doctor` until either the extension
reports connected or a bounded timeout elapses, then run one smoke tool-call. Replaces the ~15
manual tool calls this session's live-verification took with one invocation.

### 7. A committed test fixture manifest

`examples/dev-live-test.json`: schema 3, one sacred domain, one scoped grant -- the exact shape this
session hand-wrote from memory and got wrong on the first attempt (missing `schema: 3`). Reused by
`scripts/dev-loop` and citable directly for any future manual live test.

## Consequences

- All three instrumentation gaps this session hit are closed for every FUTURE live-debugging
  session, not just retroactively explained for this one.
- `lightbox fake-browser --auto-reply`'s billion-scale canned tabId is a standing regression guard:
  any future change to the composite-tabId encoding gets exercised against a realistic magnitude on
  the very first offline test run, before a real browser is ever involved.
- Nothing here changes shipped, production behavior: `lightbox` is `publish = false` and never built
  by `release.yml` (unchanged from ADR-0056); `scripts/dev-browser` and `scripts/dev-loop` are dev
  tooling, not installed or packaged; the extension's debug-event forwarding defaults off and only
  ever fires the new message type, never touching the existing tool-dispatch wire shapes.
