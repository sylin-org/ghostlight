# Foundry demo press_key diagnosis

Status: RESOLVED for the press_key failure; one follow-up defect (desk bell) documented at the
bottom. Started 2026-08-24, resolved same day. This ledger records what was eliminated, what
caused the failure, what was fixed, and the exact next defect with its mechanism.

## The question

`scripts/demo-foundry.ps1` failed deterministically at the `key to end` beat
(`browser_press_key`, key End, target the Release name textbox) with status failed and the
summary "The browser disconnected before anything happened." Standalone press_key calls,
short repros, and big-GIF repros all passed. Multiple failures across pacings, always the
same beat.

## Root cause 1 -- resolved: the keyboard beat targeted a control the story had hidden

Two defects stacked on top of each other:

1. **Demo script defect.** `Complete release packet` replaces the packet view. After it runs,
   the Release name textbox is no longer renderable: the extension refuses to focus it
   (`primitive_failed: target is not visible for focus`) and equally refuses a scroll-reveal
   (`target is not visible for scroll`). Every passing repro passed only because it skipped
   the completion beats and left the form visible.
2. **Presentation defect.** The orchestrator folded that honest adapter refusal into
   `BrowserError::Primitive(message)` and then rendered it through the browser-stopped
   fallthrough in `work/mod.rs`, announcing a disconnection that never happened and dropping
   the extension's message entirely. This disguise is what turned one evening of debugging
   into an investigation of transport ghosts.

### Fixes landed

- `fix(language)`: new `Refusal::BrowserPrimitive { detail }`; primitive adapter errors now
  route through `routing_refusal` with facts `{"reason":"browser_primitive_failed",
  "detail":...}` and render "The browser refused this job: <detail>." Two tests pin the
  rendering and the facts.
- `fix(demo)`: both `demo-foundry.ps1` and `demo-foundry.sh` now run the keyboard beat while
  the packet form is still on screen (after `release packet`, before `complete`), where End
  has real meaning: it carries the caret to the end of the value just typed.

Proven live end-to-end against the deployed authority: every beat through `key to end`
succeeds.

## Investigation trail worth keeping

- Concurrent `browser_tabs list` polls never cross the liveness gate or the wire; they prove
  only that the authority process is up. Tab listing is session-scoped, which is why poller
  sessions saw zero tabs while the demo held one.
- An instrumented-authority trace across a full failing run showed: zero liveness-stale
  transitions, every heartbeat acknowledged within milliseconds, all three of press_key's
  sub-dispatches dispatched AND acknowledged, failure in 32 ms. That exonerated the entire
  liveness machinery and browser resolution in one run and forced the search onto the
  rendering layer, where the real cause was hiding behind the fallthrough sentence.
- `browser_record save` crosses no dispatch probes during its encode window; recording
  export does not ride the command path.

## Machine-state rule learned en route

Live authority swaps must go through `scripts/dev-loop.ps1`. Hand-copying binaries over
`target/release` and killing processes by hand races connector demand-start, which produced
two live instances (and two workbench windows) mid-diagnosis. Recorded as a standing
preference in [../../MEMORY.md](../../MEMORY.md).

## Follow-up defect -- open: clicks into page-blocking dialogs never confirm

First exposed by this diagnosis: every earlier failure aborted the demo before the desk
stage, so the desk beats had not run since the capability-restoration click changes.

Mechanism, verified to the wire:

1. The desk stage's bell handler calls `window.prompt()` synchronously
   (`/assets/demo.js`, `on("desk-bell", "click", ...)`; the page's own comment calls it "a
   real page-blocking dialog").
2. Extension activation confirms through the content script
   (`service-worker.js activate()` -> `content(tab_id, {kind:"activate"...})`), which lives
   on the page's blocked main thread. The reply is physically impossible once the dialog is
   open.
3. No receipt and no error frame arrive; the trace shows only the orchestrator's own
   cancellation being acknowledged after the 8 s default deadline
   (`operation_timeout` fallback). `DeadlineAfterDispatch` then renders honestly as
   "Sent, but the browser never confirmed what happened." -- the rendering path is correct;
   the extension behavior is not.

Fix direction sketched (needs its own pass, owner eyes, and tests): during activation, race
the content-script reply against the debugger-side dialog-opened event that
`lib/debugger.js` already tracks browser-process-side; a dialog opening for the target's tab
during the click window IS activation evidence and can complete the receipt (subject may be
absent; `Activated.subject` is already optional). Never treat dialog-open as failure.

## Environment state

Clean. Instrumentation reverted from source; instrumented binaries deleted; the live
authority was rebuilt and redeployed through `scripts/dev-loop.ps1 -Action Deploy
-Component orchestrator` and serves from `target/release`. Throwaway probe scripts remain
only under the machine-local temp directory outside the repository.
