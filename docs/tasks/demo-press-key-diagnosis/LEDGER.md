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
2. Extension activation confirmed through the content script
   (`service-worker.js activate()` -> `content(tab_id, {kind:"activate"...})`), which lives
   on the page's blocked main thread. The reply was physically impossible once the dialog
   opened.
3. No receipt and no error frame arrived; the trace showed only the orchestrator's own
   cancellation being acknowledged after the 8 s default deadline. `DeadlineAfterDispatch`
   then rendered as "Sent, but the browser never confirmed what happened."

### Sprint fix landed (2026-08-24, commit e5b21195)

The content script now replies to `activate` BEFORE dispatching the click. Validation,
subject computation, scroll-into-view, and event planning all run first; `sendResponse`
crosses to the worker while the thread is still live; only the validated dispatch follows.
A handler that freezes the page can no longer swallow the receipt -- the orchestrator gets
its `Activated` outcome immediately and `browser_dialog` takes over from there. The event
planning lives in `shared.activationPlan` (node-tested), which also corrects a latent bug:
modified primary clicks previously synthesized `button: 2` (right-click) instead of `0`.
Pins include a behavioral ordering test (reply strictly before dispatch) and a source pin
that will fail if anyone reintroduces reply-after-dispatch.

Live verification completed 2026-08-24 after the owner reloaded the unpacked extension:
`scripts/demo-foundry.ps1` ran green through all 41 beats, including `key to end`,
`ring once`, `dialog answer`, `ring again`, and `bell silent`. One pacing nuance observed
with the early-reply design: the activation receipt can arrive a few milliseconds before
the opened dialog becomes observable, so a fast `dialog status` immediately after a click
may legitimately report no dialog yet; the demo's beats still pass because each beat
reports truthfully.

## Sprint companions landed the same day

- `a7c7da4f` -- effect-unknown truthfulness: adapter `EffectUnknown` receipts route through
  the unknown rendering with the browser's own reason in facts (`detail`) instead of
  per-family receipt matching calling them incompatible receipts; an exhaustive table test
  pins that only a true pre-dispatch disconnection may claim disconnection, and exactly the
  four after-dispatch classes claim unknown effects. The audit now records bounded refusal
  facts for every non-success terminal (`refusal_facts`) and stays free of them on
  successes, whose facts can carry page-derived values.
- RELEASE-CHECKLIST G1 gained the whole-catalog foundry demo as a standing candidate gate,
  required again after any input-path, extension, or browser-relay batch.

## Environment state

Clean. Instrumentation reverted from source; instrumented binaries deleted; the live
authority was rebuilt and redeployed through `scripts/dev-loop.ps1 -Action Deploy
-Component orchestrator` and serves from `target/release`. Throwaway probe scripts remain
only under the machine-local temp directory outside the repository.

## Linux live verification -- 2026-08-24

The [dated CachyOS record](../../testing/foundry-linux-live-verification-2026-08-24.md) proves the
same sprint at source revision `793e258`: all source gates passed, the exact optimized three-sibling
user candidate and explicitly reloaded unpacked adapter were active, all 41 normal-paced foundry
beats passed, and the decisive primitive refusal plus failure-only audit facts rendered correctly.
No Linux product defect appeared. This remains development-host evidence, not package or release
evidence.
