# Quiet coexistence local engineering trial

Status: candidate checkpoint awaiting remaining validation. No completion or release claim.

## Source and authority

The initial checkout was clean at
`de1a686761af5430afc50763d1a282efaa80f615`. There was no pending-change checkpoint to commit.
The trial uses branch `codex/quiet-coexistence-trial` in
`E:\repo\github\sylin-org\ghostlight`. Existing worktrees were preserved.
Local engineering commits and disposable local installation/profile tests were authorized.
The selected everyday installation, production preferences, public versions and remote branches
are outside this trial's mutations. The owner requested a candidate checkpoint before the
remaining live verification; later corrections belong in follow-up commits. The exact checkpoint
is reported with the handoff and in the branch log, without claiming the entire suite passed.

## Problem and chosen design

An operator and several agents share a browser. Foreground tab creation, same-host adoption,
duplicate-group movement and native input can disturb the operator even without an explicit
window-focus call. An inactive-tab acknowledgement can also conceal missing native effects.
[Chrome DevTools MCP issue 2856](https://github.com/ChromeDevTools/chrome-devtools-mcp/issues/2856)
describes the need for operator control over agent focus changes.

[ADR-0186](../adr/0186-quiet-browser-coexistence.md) records one persisted operator attention
setting, a shared executor admission rule, negotiated adapter mechanics and existing Workbench
reveal. No scheduler, workflow engine, attention service or second operator surface was added.
Background admits safe work; Pause and Stop still govern future dispatch. Show tab reveals a
currently owned tab and grants no permission, resumes no session and replays no work.

The default is background. Foreground is an explicit operator opt-out that also restores
unowned same-host reuse and historical group repair. Effective results explain the setting's
value, deciding layer and organization ceiling. A queued foreground request refuses if the
operator tightens the rule before actual writer admission. CLI, MCP and flow children share
these decisions. Old adapters cannot claim background enforcement without its capability.

Existing-browser connection and scripting are not unique to Ghostlight:
[Playwright MCP](https://github.com/microsoft/playwright-mcp) documents its Chrome extension
for existing logged-in tabs, and [Playwriter](https://github.com/remorses/playwriter) documents
CLI/MCP scripting, shared tabs and per-session tab groups. This trial concentrates on shared
operator authority, meaningful receipts and recoverable coexistence; it does not claim a new
workflow engine or measured superiority in token use.

## Changed and retired behavior

- Background opening protects unowned human tabs and matching duplicate groups. Work-window
  selection requires an unfocused window with only owned tabs; otherwise a new unfocused window
  is allocated without moving existing tabs. Native surface coordination is being completed.
- Background focus, shared-window resize and unsafe active close have distinct typed refusal
  reasons. A protected close compensation retains a known applied effect instead of reporting
  an unknown effect. Browser recovery requests human launch under background attention.
- Legacy custody release uses bounded Ended retirement, never a physical close fallback. An
  incompatible connection cannot regain Active state through reconnect. Modern new service
  epochs release debugger custody and forget stale ownership associations; same-epoch recovery
  retains live continuity. Browser groups, titles and placement are preserved.
- Workbench exposes the setting and exact owned-tab Show tab on its existing surface. Flow
  child receipts clear earlier child selection evidence. Historical rows do not invent a live
  reveal destination. No automatic denial or credential hold was reintroduced.
- Background textual form fill uses one native replacement with exact selection and validated
  blur commit, then verifies the complete batch's retained values. Native acknowledgements do
  not establish retention. Explicit key semantics remain separate from whole-value editing.
- Both scripting recipes use a fresh tab and stop at the first unsuccessful receipt. The
  PowerShell recipe waits for the GUI executable and reads its actual process exit status.
  Unknown effects are retained as the first failure rather than hidden by a later close.
  CLI stdin continuation and explicit flow continuation remain compatible and documented.
- The duplicate command-to-tab mapping was consolidated onto the shared typed command method.
  Active child-tab adoption claims were reconciled with ADR-0164. Historical decisions and
  product material remain in place with marked amendments where needed.

## Verification record

Final gates and installed journeys are pending. The following completed diagnostic evidence
is retained so a later successful run cannot hide earlier failures.

| Run | Result | Evidence |
| --- | --- | --- |
| First full hardening suite | 21 passed, 4 failed; unchanged source | `.tmp/hardening-suite/2026-09-30T22-50-54-094Z-20556/results.json` |
| Rust review fixes | 587 tests passed; subsequent native coordination changes not covered | `.tmp/quiet-trial-evidence/review-fixes-rust.log` |
| Fresh review-fix build | Three workspace executables built; not the final candidate | `.tmp/quiet-trial-evidence/review-fixes-build.log` |
| Recipe runtimes | 9 PowerShell and 9 POSIX cases passed | `.tmp/recipe-prerequisites/verification.txt` |
| Screenshot environment isolation | Correct background pixels; human focus unchanged | `.tmp/capture-probe-1790812635011/results.json` |
| Integrated full suite | 24 passed, 4 failed; unchanged source | `.tmp/hardening-suite/2026-10-01T00-08-07-961Z-35060/results.json` |
| Group placement, text and two-client native effects | Actual trusted values/effects passed; later refusal mapping failed | `.tmp/quiet-browser-2026-10-01T00-25-53-627Z-33200.json` |
| Historical custody and restart | Passed actual pinned adapter; pre-final copied candidate siblings | `.tmp/quiet-browser-legacy-custody-2026-10-01T00-30-58-900Z-45908.json` |
| Scoped native-refusal correction | One regression passed; fresh three-sibling build | `.tmp/quiet-trial-evidence/scoped-native-refusal-fixed.log` |

The first suite exposed a real form-input retention failure, two screenshot timeouts, and a
legacy fixture expectation that did not match passive presentation frames. These are not green
results. The form failure led to direct retained-value and trusted-input comparisons. Native
keyboard/pointer probes also exposed inactive-tab success acknowledgements without effects,
which requires the mechanical window coordination described in ADR-0186.

The capture probe timed out even on a focused visible synthetic document. On this Windows
desktop, one test-browser flag, `--disable-backgrounding-occluded-windows`, restored capture.
The successful background image contained the red agent fixture; the separate blue human
document retained its URL, visibility and focus before and after. The shared Chromium fixture
helper applies this flag on Windows and the installed fixture discloses it, including its
WebView screenshot arguments. Production browser launch behavior is unchanged. This proves
capture in the declared fixture environment, not capture on every ordinary desktop setup.

The integrated run exposed an additional topology error. Chrome's
[new-group API](https://developer.chrome.com/docs/extensions/reference/api/tabs#method-group)
defaults to the current window when `createProperties.windowId` is omitted. New unfocused work windows
were therefore undone by grouping: the agent tabs moved into the human window, while the human
tabs stayed in place. The corrected implementation must name the actual work window, recheck
placement and return a refreshed tab. Acceptance must observe both human and agent placement.
Diagnostic evidence: `.tmp/quiet-browser-2026-10-01T00-15-14-032Z-43816.json`.

The next actual native-input check exposed a completion-path error: a document-scoped native
refusal was unwrapped but not mapped to the typed attention refusal. It appeared as an
incompatible receipt. The shared completion seam now handles that inner result; its regression
exercises the document envelope and preserves blocked/no-effect facts and coverage.

At this checkpoint the current quiet journey, complete installed Workbench controls and cleanup,
full-page capture after the grouping correction, and final exact-source gates remain outstanding.
The browser specialist is the sole active live-test owner. Partial transport, typing and Pause
proofs do not establish that the remaining Show tab, Resume, Stop and cleanup checks passed.

The full acceptance run must use fresh siblings in `.target-quiet-trial/debug`, both optional
compatibility and native-installation lanes, local synthetic pages and exact source hashes.
Receipts must be compared with observed tab/window focus, retained input and actual custody.
The installed proof uses a disposable user/profile, a unique native-host registration and exact
copied executables. Production registry and manifest snapshots must match before and after.

## Limits and follow-up

Direct Ghostlight mechanisms cannot contain page-originated popups, dialogs or script focus.
Creating an unfocused work window may still make a window visible. Browser API observations
and native effects are not one atomic OS transaction. Relevant change fences bound refusals
and uncertainty; they do not promise an impossible race-free desktop.

Legacy Ended delivery has no correlated cleanup acknowledgement. Old cached topology hints
cannot be claimed forgotten merely because debugger retirement was requested. Preserved tabs
can accumulate across released sessions. No exactly-once or token-saving claim is made.

The disposable installed journey is distinct from the owner's everyday installation, store
packaging and Linux runtime evidence. Those latter scopes have not been tested by this trial.
Independent review of the exact final commit remains required before presenting completion.
