# Continuation record: retained form-fill fix verified

The implementation and installed verification are complete. The older handoff is preserved below
as investigation history; do not restart its superseded experiments. The owner subsequently
authorized a version bump and Chrome Web Store deployment on September 13. Adapter 1.1.6
release progress is recorded in docs/STATUS.md. A subsequent multiline report exposed
missing Enter character payloads. The correction is committed as `8f2d1981`; 267 tests
and six real Chromium editing cases pass. Adapter 1.1.7 replaced the cancelled 1.1.6
review and is pending Google review with automatic publication enabled. The installed
unpacked extension has not yet been reloaded for this multiline correction.

## September 13 implementation update -- scoped focus approved

The owner approved the Ghostlight-side fix after the controlled comparison: focus emulation
follows controlled debugger tabs while runtime control is active. ADR-0168 records the decision.
The source is implemented and all 265 extension tests pass. Attach/focus/detach commands are
serialized per tab; pause, attention, disconnect, stop, local debugger release, and preserved
workspace release restore normal behavior. No form is submitted and no site code is patched.

The first installed attempt after reload exposed a missing adapter ownership cache. Chrome clears
`storage.session` on reload, while the orchestrator keeps workspace ownership. The correction
relearns the opaque association from explicit service requests, without regrouping/moving tabs.
The new regression starts with an empty cache and proves focus custody is restored. The owner
completed the second reload. Installed Ghostlight then filled all five fields without submission
and retained every DOM and React Hook Form model value through an 81.465-second inactive-tab
interval, return, field click, and another 29.159 seconds. No Codex browser/CDP focus override or
diagnostic page hook was added. The verified draft remains unsaved. See
`docs/testing/controlled-tab-focus-installed-2026-09-13.json` for the exact source hashes and evidence.

Formatting, Clippy, and the full Rust test suite passed serially in `.target-form-retention`.
All 265 extension tests, changed-JavaScript syntax, ASCII documents, and diff checks passed.
No parallel deployment, form submission, site patch, or publication occurred. Pause, stop,
disconnect, and release cleanup are covered by lifecycle/worker regressions; the final unsaved
profile was not released to trigger its known site reset. The browser-control tool's restriction
on `chrome://extensions` remains binding for any future reload; do not work around it.

The older statement below that focus emulation has no product authorization is superseded by
this decision. Permanent unsaved-draft retention after control ends remains outside the promise.

## September 13 evidence update -- read before the older handoff below

The controlled follow-up is complete. Both Ghostlight and Codex retained all five fields with
focus emulation enabled and lost them after a normal tab return with it disabled. The decisive
Codex run reset without a field click. The site's visibility/focus handlers refresh user data;
the refreshed object retriggers the profile-loading effect, which fetches saved values and calls
React Hook Form `reset()` without preserving dirty fields. The reset occurred 502.5 ms after the
visible event. See `docs/testing/form-retention-focus-comparison-2026-09-13.json` and Research 29.

The input-only diagnosis and the recommendation to repeat keyboard experiments below are stale.
This remaining retention failure needs a site-level initialization/dirty-value fix. Indefinite
focus emulation only masks it and is not authorized as a permanent product change. No site code
was changed, no form was submitted, and all added debugger logpoints/probes/focus overrides were
removed. Product code was unchanged in this follow-up; the existing uncommitted work remains.

The original input-only diagnosis below is superseded. Read
`docs/research/29-form-retention-focus-and-frameworks-2026-09.md` and the latest `docs/STATUS.md`.
Native keyboard replacement reached all five actual React Hook Form model values, then the page
later restored saved values through registration/ref callbacks. Whole-value native insertion
also reached the model and later failed retention. The caller is now identified above. No site
fix has been applied.

The proposed document-local focus correction below has been implemented and the owner reloaded
installed source 1.1.5. All 246 extension tests passed afterward. Do not repeat this implementation
step or use its test success as evidence that the full live retention journey passes.

Codex browser control kept an inactive tab reporting visible/focused. Disabling focus emulation
restored normal readings; the next real focus cycle exposed model loss. Playwright has matching
documented behavior. A real React/RHF fixture also showed that stale props instrumentation can
log only the first input even though the live model receives later input. Do not assume the old
first-character trace proves missing React updates.

The focus comparison and reset-caller tracing are complete. No form may be submitted. No parallel
deployment is authorized.
The owner authorizes modifying and reloading the installed extension through computer control;
do not ask for permission already supplied. Respect actual tool policy blocks if encountered.
The passive profile probe has been removed and the temporary fixture server stopped. The ignored
fixture source remains under `.tmp/form-input-investigation/`. No commit or publication was made.

## Older handoff -- preserve scope, re-check diagnostic conclusions

Work in `E:\repo\github\sylin-org\ghostlight` and finish the active Ghostlight 1.1.5 form-fill bug. Persist through implementation, live verification, documentation, full gates, and a local commit. Do not push, publish, submit the web form, or touch the untracked `.zcode/` directory.

Read the repository `AGENTS.md` and its required documents first, especially `docs/MEMORY.md`, `docs/STATUS.md`, the four `docs/1.0/` contracts, the relevant ADRs, and `docs/DEV-LOOP.md`. ASCII only applies to code and documentation. Do not read files under `local/` without fresh owner authorization.

## User-visible bug and required result

The affected page is:

`https://hackathon.genai.works/profile#profile`

`browser_fill_form` appears to fill the profile and shows the unsaved indicator. About 40 seconds later, the page's React state restores the saved values. Switching away and back makes the loss obvious. The final implementation must fill without submitting, retain every filled value through at least a 45-second tab switch, and retain them after clicking back into a field.

Use harmless test values such as:

- First name: `Ghostlight`
- Last name: `Final Verified`
- Current position: `Trusted Browser Input`
- City: `Live Test Location`
- Bio: `Unsaved Ghostlight 1.1.5 final verified retention test.`

Never supply `submit_target` and never click `Save all changes`. Leave the verified draft unsaved for the owner to inspect.

## Repository state

Current branch: `main`

Current HEAD: `140728e4 docs(research): inspect ChatGPT browser extension`

There is one broad uncommitted change. Inspect it before editing:

```text
 M crates/orchestrator/src/work/mod.rs
 M extension/content.js
 M extension/lib/documents.js
 M extension/lib/shared.js
 M extension/manifest.json
 M extension/service-worker.js
 M extension/tests/content.test.js
 M extension/tests/form-diagnostics-routing.test.js
 M extension/tests/shared.test.js
?? .zcode/
?? extension/tests/click-worker.test.js
?? extension/tests/diagnostic-navigation.test.js
?? extension/tests/fill-worker.test.js
?? extension/tests/keyboard-worker.test.js
```

Do not discard the useful diagnostic, click, load-ready, or version work while fixing fill. Do not touch `.zcode/`.

The manifest is already bumped from 1.1.4 to 1.1.5. The owner has repeatedly reloaded the unpacked extension after source changes. Ask for another reload only after a concrete new extension change is tested locally.

## Proven findings

The failure is same-document React reconciliation, not navigation, form reset, node replacement, or the browser tab switch itself.

- Four synthetic fills were replaced together about 40.25 seconds after the fill.
- The same document and the same control nodes remained.
- No `input`, `change`, `reset`, navigation, or network event accompanied the later erase.
- React wrote the saved values from its model.
- A native physical edit survived the later render in an earlier A/B test.
- The page uses React Hook Form. Its `onChange` and `onBlur` handlers were observed directly.
- The page can show an unsaved indicator even when its durable in-memory model did not receive the complete replacement.

Do not spend another session attributing this to visibility, reload, delayed API hydration, or a site-wide defect. The input mechanism is the active defect.

## Input experiments already completed

1. Prototype value setter plus synthetic `input` and `change` events looked correct immediately but lost all values at the 40-second React render.
2. A hybrid CDP sequence using one physical character followed by `Input.insertText` produced the full DOM value, but React's `onChange` saw only the first character.
3. Per-character `Input.dispatchKeyEvent` using incomplete printable descriptors produced trusted `keydown`, `beforeinput`, `input`, and `keyup` events and the full DOM value. React still forwarded only the first character, then restored the saved value about 200 ms later.
4. The incomplete descriptors had no printable `code`, Windows/native virtual key codes, Shift state, or `unmodifiedText`. This contradicted accepted ADR-0088 and Ghostlight's own pre-rewrite keyboard domain.
5. A transient attempt to copy the short-term behavior of Codex browser control's `setValue` used the prototype setter again. It passed immediate verification and all 244 extension tests, but the live 45-second check failed exactly as before. That path has been removed again.
6. The current source restores complete printable descriptors modeled on Ghostlight's own accepted and previously implemented ADR-0088 design. Letters and digits now carry physical codes and virtual key codes. Uppercase and shifted punctuation carry Shift. Printable keydown packets carry both `text` and `unmodifiedText`. Modified shortcuts omit both text fields. `Ctrl+A` previously inserted a literal `a`; the current `pressKey` correction fixes that too.

The official ChatGPT Chrome extension package was already downloaded and studied. See `docs/research/28-chatgpt-browser-extension-2026-09.md`. It exposes `set_value`, `type_text`, and locator fill commands but delegates the actual fill algorithm to the proprietary native runtime. Do not repeat the package download or pretend the extension contains the host-side algorithm. Ghostlight's own ADR-0088 and historical `extension/lib/keys.js` implementation are legitimate project authority. Commit `6c0dc4d8` contains that prior project-owned packet planner for reference.

## Current implementation and immediate next step

The current work-in-progress does the following:

- `extension/lib/shared.js::keyDescriptor` builds complete physical descriptors for printable ASCII.
- `extension/service-worker.js::replaceFocusedText` uses trusted CDP `Ctrl+A`, per-character keydown/keyup, then Tab to commit blur.
- `pressKey` combines descriptor Shift state with requested modifiers and removes text fields from command shortcuts.
- `browser_fill_form` validates all fields first, uses a trusted pointer click to focus each textual field, types it, and verifies exact value retention for 250 ms.
- `extension/content.js::prepareStableFill` waits for a complete/interactive document, a minimum six-second document age, and a stable target-value signature before mutation.

The most recent live run did not test the complete keyboard descriptors. It failed before typing with:

`target did not retain browser input focus`

The tab was visible, but no field was active. The trusted CDP pointer focus transition is now the blocker. A patch was attempted but failed to apply because it mixed `content.js` and `service-worker.js` context. No part of that failed patch was applied.

Start with this narrow correction:

1. In `extension/content.js::prepareBrowserText`, scroll, call `element.focus({ preventScroll: true })`, verify `deepestActiveElement() === element`, and return the subject. Geometry is no longer needed for form focus.
2. In the `browser_text` branch of `extension/service-worker.js::fill`, remove `frameViewportOffset` and `dispatchClick`. Call `prepare_text_fill`, verify focus, then call `replaceFocusedText`.
3. Update `extension/tests/fill-worker.test.js` so it proves validation, document-local focus preparation, complete printable packets including `unmodifiedText`, Tab before value verification, and debugger cleanup. The separate semantic-click test must continue proving that ordinary `browser_click` uses a trusted pointer.

Do not remove the trusted-pointer semantic click correction. A page-generated `element.click()` previously erased a retained draft when the user clicked back into the form. `browser_click` now resolves live geometry in content and dispatches the pointer through CDP in the worker. That behavior has its own `click-worker.test.js` coverage.

After the focus correction, run focused extension tests and JavaScript syntax checks. Ask the owner to reload 1.1.5 only if source changed. Then run one clean live test. The 250 ms per-field verifier should immediately reject an incomplete React update, so do not begin the 45-second wait unless `browser_fill_form` itself succeeds.

If the complete descriptors still cause React to forward only the first character, compare the current packets directly with the project-owned `textDispatchPlan` in commit `6c0dc4d8:extension/lib/keys.js`. Check `type`, `key`, `code`, `modifiers`, virtual key codes, `text`, `unmodifiedText`, and keyup modifier state as one packet contract. Fix the packet planner rather than adding site-specific waits, synthetic events, or React internals.

## Live verification recipe

The prior Ghostlight workspace had a profile tab and an Example Domain tab, but handles are workspace-scoped and may change in a new session. List or adopt the current tabs rather than assuming these handles remain valid.

1. Focus and reload the profile page through Ghostlight.
2. Fill the five fields above with `browser_fill_form`, no submit target.
3. Wait five seconds and read the five DOM values plus the unsaved indicator.
4. Focus an unrelated tab for at least 45 seconds.
5. Focus the profile tab and verify all five values before clicking anything.
6. Use `browser_click` on the first-name field.
7. Verify all five values again and confirm the unsaved indicator remains.
8. Leave the profile tab visible and unsaved. Do not submit.

The last synthetic live test filled all five fields and passed the five-second check, then all five reverted to the saved profile at the 45-second check. That is the exact regression gate.

## Load-ready work that must remain

The user also asked to determine why `load_ready` failed.

- One cache-bypassing reload did not reach interactive within the 30-second wait. The precise page blocker cannot be reconstructed because the previous adapter erased its diagnostic ring on navigation.
- The extension now preserves an explicitly enabled diagnostic lease across navigation, starts a fresh volatile ring at `onBeforeNavigate`, and adopts the new document on the first diagnostic read. A later hard reload completed in about 0.6 seconds and retained 97 navigation entries.
- All page documents, scripts, fonts, and application APIs were successful. Only blocked PostHog analytics requests and normal canceled Next.js prefetches appeared; neither gates readiness.
- The orchestrator reserved only 250 ms for an unsatisfied wait receipt to cross the extension, native relay, browser port, executor, and MCP edge. That was too small and converted a normal false load-ready result into an uncertain after-dispatch deadline.
- `WAIT_RECEIPT_RESERVE_MS` is now 750 in `crates/orchestrator/src/work/mod.rs`, with the expected adapter wait changed from 2750 to 2250 in its unit test.

Keep those changes and verify them with the Rust gates. Describe the historical load failure honestly as a transient page load whose exact old blocker is unavailable, plus a Ghostlight receipt-budget bug and a diagnostics-loss bug that are now corrected.

The release orchestrator was already rebuilt and deployed locally with:

```powershell
pwsh -NoProfile -File scripts/dev-loop.ps1 -Action Deploy -Component orchestrator -Profile release -TargetDirectory .target-form-retention
```

It completed successfully and restarted `target/release/ghostlight.exe`. Redeploy again only if the Rust source changes further or the running binary no longer matches.

## Verification and finish

The most recent focused extension run after restoring complete key descriptors passed 92 tests. The full suite has not been rerun after that final descriptor change. Run all required gates after live success:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd extension && npm test
node --check on every changed extension JavaScript file
git diff --check
```

Run Cargo commands serially. A prior parallel Cargo run caused a linker collision. For process journeys, remember that `tests/process-journey.mjs` defaults to `.target-ghostlight-1.0/debug`; pass `GHOSTLIGHT_BIN_DIR` if using another target directory.

After the live regression passes, update active documentation to match the final mechanism. At minimum inspect and amend, without rewriting history:

- `docs/adr/0088-browser-input-event-fidelity.md`
- `docs/adr/0138-frame-transparent-semantic-layer.md`
- `docs/1.0/LANGUAGE.md`
- `docs/1.0/ARCHITECTURE.md`
- `docs/1.0/ACCEPTANCE.md`
- `docs/testing/form-reset-diagnostics-2026-09-12.md`
- `docs/STATUS.md`
- `docs/MEMORY.md` if the final learning is durable and cross-cutting

ADR-0138 currently says DOM-level actions need no coordinates. The demonstrated semantic-click correction is frame-aware: content resolves and scrolls the exact target and returns live geometry; the worker applies frame offsets and sends a trusted CDP pointer. Append an amendment rather than silently contradicting the accepted text.

Review the complete diff, preserve one logical change, and commit locally with a conventional signed commit such as:

`fix(browser): preserve controlled form drafts`

Do not push or publish. Report the root cause, exact live evidence, load-ready finding, gates, and local commit hash. State explicitly that the form was not submitted.
