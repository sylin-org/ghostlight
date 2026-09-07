# H5: Session recovery and dependable runtime control

Status: implemented, verified, and deployed locally on 2026-09-07. Owner approval: 2026-09-06.
The [deployment record](../../STATUS.md#local-deployment-2026-09-07) records the exact binary
and live reconnection checks. Installed-browser control timing remains untested.

## Accepted behavior

Explicit request restrictions remain enforcing when ordinary policy observes. Automatic attention
is local to its session. Existing global Pause and Stop retain their scope and fixed language.
Pause is checked at dispatch after preparation; already dispatched work keeps its effect evidence.

Keep three matching enforced denials in 60 seconds or five in 120 seconds. Observe findings do not
count. One attention notice identifies the session and leads to its grouped history. Review and
explicitly resume that session; no cooldown, automatic replay, permission expansion, or extra
settings. Global Resume preserves session attention. Resume only permits new requests.

## Implementation

One logical commit repairs admission, stores attention with workspace lifetime, adds a final
browser-port admission callback, and uses the existing workbench and Tauri human-control path.
A stale recovery action cannot clear a newer attention incident. Credential handoff uses the same
session attention state with its own reason. A deliberately global runtime-control-file attention
state remains global. No connector protocol or extension policy change is required.

## Verification

- Formatting and workspace Clippy with warnings denied pass. All 469 Rust tests pass, including
  392 orchestrator library tests. All 183 extension tests and changed JavaScript syntax pass.
- Six new Rust regressions prove direct/composed observe restriction enforcement, two-session
  isolation, thresholds and scoped recovery, stale incidents, global Pause/Stop independence,
  refusal after preparation, queued writer/cancellation/deadline boundaries, and truthful applied
  or uncertain receipts. Existing threshold window/reset tests remain green. An incident has its
  own id so a stale action cannot clear a newer incident in the same composition.
- The fresh isolated `.target-ghostlight-1.0/debug` process journey passes through real MCP,
  orchestrator, and browser connector executables with synthetic native framing. It checks the
  actual MCP error flags and JSONL child records, an unaffected second session, global Resume
  preserving attention, and Pause acknowledged between describe and attempted input. Presentation
  frames remain allowed; no input command reaches the adapter.
- `tests/workbench-surface.mjs` passes. The bundled UI in isolated Chromium passes history
  expansion, first-problem scrolling, preserved focus/scroll, review after Clear view, exact
  incident recovery, and the 720-pixel layout. The review screenshot was inspected.
- Not run: installed Tauri/MV3/native-host browser timing, visible physical input, or Linux runtime
  lanes. No claim of complete live integration follows from the synthetic process or UI evidence.

The interrupted patch already held the accepted architecture. Completion also fixed stale
credential/global-attention assertions, made the triggering child stop even if recovery races its
completion, and retained confirmed action effects when postcondition observation cannot dispatch.
Close compensation uses the final guard. The history cache retains current incremental receipts
for later explicit review instead of resurrecting older snapshot data.

Commit: `fix(control): isolate session attention and guard browser dispatch` (use Git for hash).
Next: H6 ideation I5; do not implement its unresolved contracts without that discussion.

The owner separately authorized local deployment on 2026-09-07. No push, publication, or
local-machine notes access was authorized or performed.

Completed landings are still checked against policy after dispatch. A later runtime control
change does not create a permanent policy hold on an otherwise permitted tab; the next command
checks the live control again. Recovery tests prove the same tab remains usable after Resume.
