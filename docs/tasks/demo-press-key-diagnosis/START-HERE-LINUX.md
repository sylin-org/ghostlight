# START HERE -- Linux verification lane for the foundry sprint

This file is self-contained and assumes you remember nothing. Work top to bottom. One task,
one logical commit set, one green tree. If a task cannot complete, revert your change, mark
it BLOCKED in this file's task list with the reason, and stop rather than improvising.

## Who you are and what this lane is

You are the agent on the CachyOS development host (KDE Plasma, Wayland, Chromium). The
Windows side landed a same-day diagnosis-and-fix sprint for two foundry-demo defects and is
asking you to prove them on Linux against a real visible browser. Nothing here is release
publication work.

## What changed on the Windows side (dev head as of this brief)

Read `docs/tasks/demo-press-key-diagnosis/LEDGER.md` for the full story. Summary:

1. The foundry demo's press_key beat used to fail because it ran after `Complete release
   packet` replaced the packet view; the extension honestly refused an invisible target but
   the orchestrator rendered that as a phantom disconnection. Fixed on both sides:
   `scripts/demo-foundry.ps1` and `demo-foundry.sh` now run the keyboard beat before
   completion, and primitive adapter refusals render the browser's own reason
   (`Refusal::BrowserPrimitive`, facts `browser_primitive_failed` + `detail`).
2. Extension activation now replies to the service worker BEFORE dispatching the click
   (`extension/content.js`, planning helper in `extension/lib/shared.js
   activationPlan`). A click whose handler opens a page-blocking dialog (`window.prompt`,
   confirm, alert) can no longer swallow the receipt -- previously the desk-stage bell beat
   hung 8 seconds and ended "sent, but never confirmed". `activationPlan` also fixes a latent
   bug: modified primary clicks synthesized `button: 2` instead of `0`.
3. Effect-unknown receipts now render truthfully with the browser's own reason in facts, and
   audit records carry bounded `refusal_facts` for every non-success terminal.
4. RELEASE-CHECKLIST G1 gained the whole-catalog foundry demo as a standing gate.

All of this is gated on Windows: 329 Rust tests, 132 extension tests, fmt, warnings-denied
Clippy, and one full live run of all 41 demo beats after reloading the unpacked extension.

## Your tasks, in order

### T1. Sync to the lane head

`git fetch origin dev` and put your working tree exactly at origin/dev (fast-forward; if you
carry local dirt, reconcile it without discarding unrelated work). Record the exact HEAD
sha in your evidence. Do not merge main, tag, publish, or release.

### T2. Source gates

From the repository root:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `npm test --prefix extension`

All four must pass before you deploy anything. Fix nothing unless something genuinely fails
on Linux; a failure here is a finding, not an inconvenience.

### T3. Rebuild, deploy the user-level candidate, reload the extension

Use your established user-candidate procedure for this host (optimized siblings, user
install under `~/.ghostlight/bin/<version>`, unpacked adapter from repository source).
Two requirements are not optional:

- The orchestrator AND both connectors must come from this exact source revision.
- You MUST explicitly reload the unpacked extension at `chrome://extensions`. The content
  script changed; without the reload the desk-stage beats will hang exactly like the old
  defect, which would be the pre-fix behavior, not a new one.

### T4. The whole-catalog foundry demo, end to end

Run `scripts/demo-foundry.sh` (normal pacing) against the ordinary visible Chromium profile.
Required result: every beat green through the desk stage -- specifically `key to end`
succeeds mid-story, and `ring once`, `dialog status`, `dialog answer`, `bell answered`,
`ring again`, `dialog dismiss`, `bell silent` all succeed at the end. A hang or an unknown
status on any bell beat is a regression of the blocking-dialog fix: diagnose it, fix it at
the owning seam with regression coverage, and rerun the whole script.

Known honest nuance, not a defect: with reply-before-dispatch, the click receipt can land a
few milliseconds before the opened dialog becomes observable, so a fast `dialog status`
immediately after `ring once` may truthfully report no dialog yet. The beats still pass.

### T5. Honest-rendering spot checks

One deliberate refusal and one audit line, so Linux evidence covers today's language fixes:

- Trigger any primitive refusal (for example, attempt a click whose target is gone) and
  confirm the CLI result says "The browser refused this job: ..." with facts
  `browser_primitive_failed` plus a `detail`, NOT a disconnection sentence.
- Show one failing record in the audit JSONL carrying `refusal_facts`, and confirm success
  records do not have the field.

### T6. Evidence

Add a dated record under `docs/testing/` following the existing CachyOS record format
(environment block, what ran, results, limits), link it from
`docs/tasks/demo-press-key-diagnosis/LEDGER.md` with one short section, and update the
STATUS.md in-flight paragraph if anything there became stale. Commit task changes separately
as they land.

## Boundaries

No main merge. No tag. No publish, upload, store action, or release of any kind. No network
behavior added to the product. No secrets, page content, screenshots, or recordings in
coordination files or committed records.

When done: replace `coordination/RESULTS.md` with your concise result, append the next
numbered message to `coordination/CHAT.md`, commit coordination files separately, push dev.
