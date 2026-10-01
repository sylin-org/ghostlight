# Quiet coexistence local engineering trial

Status: implementation and first verification complete; awaiting independent exact-commit execution.
No release or production-installation claim.

## Source and authority

The initial checkout was clean at
`de1a686761af5430afc50763d1a282efaa80f615`. There was no pending-change checkpoint to commit.
The trial uses branch `codex/quiet-coexistence-trial` in
`E:\repo\github\sylin-org\ghostlight`. Existing worktrees were preserved.
Local engineering commits and disposable local installation/profile tests were authorized.
The selected everyday installation, production preferences, public versions and remote branches
are outside this trial's mutations. The owner requested a candidate checkpoint before the
remaining live verification; later corrections belong in follow-up commits. The exact checkpoint
is `8c8d856cdc3eca635f69c1a0a21c6b632687e1a7`, without claiming the entire suite passed.

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
  is allocated without moving existing tabs. Native surface coordination is scoped to each window.
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

The following diagnostic evidence is retained so successful verification cannot hide earlier
failures. The final record follows this history.

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
| Final full suite | 26 passed, 2 new fixture expectations failed; unchanged source | `.tmp/hardening-suite/2026-10-01T01-23-57-706Z-21224/results.json` |
| Complete quiet after status-contract correction | Passed 14 observed checks | `.tmp/quiet-browser-2026-10-01T01-29-59-469Z-48848.json` |
| Complete isolated native installation after status-contract correction | Passed 5 installed journey stages; cleanup verified | `.tmp/quiet-native-installed-2026-10-01T01-30-41-124Z-29976.json` |

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

The checkpoint deliberately preceded complete live verification. Those remaining checks are
now covered by the final full suite and the two complete supplemental journeys below.

After the checkpoint, current-source standalone checks passed: workspace formatting, strict
all-target Clippy, all 588 Rust tests, all 342 extension tests, syntax checks on 33 changed
JavaScript files, and whitespace validation. The PowerShell and POSIX recipes passed nine cases
each. Logs are in `.tmp/quiet-trial-evidence/final-{format,clippy,rust,extension,recipes,policy-grammar}.log`.
These checks do not replace the outstanding live acceptance gates.

Both current quiet and native-installed fixtures reached a focus-cleanup assertion that required
investigation. An inactive controlled page reports `document.hasFocus()` true after Pause. A
bounded installed probe confirmed successful real focus-emulation disable commands for both
owned tabs, another direct disable, and debugger detachment. Both MAIN scripting and direct
CDP observation still reported focus true on the hidden page. This is not proof of retained
debugger custody or a passing lifecycle journey. Probe evidence is
`.tmp/quiet-native-installed-2026-10-01T01-00-43-152Z-48040.json`.

A separate comparison with the pinned `de1a686` adapter reproduced that reading after Hold
and Stop. More decisively, an ordinary never-emulated inactive tab initially reported focus
false, then reported true after real `Input.insertText`, even after disable and detach. Its
native `Document.prototype.hasFocus` was not replaced. This identifies inherited Chromium
native-input focus reporting, separately from quiet operation. Evidence is
`.tmp/quiet-browser-legacy-custody-2026-10-01T01-09-10-497Z-38380.json`.
Cleanup acceptance uses an inactive retained sentinel with no native input,
while preserving native-filled page state and this limitation. No production focus workaround
or desktop refocus is justified by that comparison.

The later logged control passed all diagnostic conditions, not product acceptance:
`.tmp/focus-origin-control-2026-10-01T01-23-16-569Z-30588.json`. An ordinary never-emulated tab
remained inactive in the same unfocused work window, with constant top-document and CDP target
identities. It reported focus false before and after attachment and after DOM focus, then true
after native insertion, blur, acknowledged disable and actual detach. Pass-through Chrome
command logging proved it never received an enable command. Raw WebSocket CDP outgoing logs
contained no focus-emulation override. The pinned baseline adapter reproduced the filled-page
reading after Hold and Stop in the same arrangement. This distinguishes native focus reporting
from emulation cleanup; it is not a Chromium defect claim.

Neither fixture nor observer uses Playwright or Puppeteer. The
[Playwright CDP defaults](https://playwright.dev/docs/api/class-browsertype#browser-type-connect-over-cdp)
and its `noDefaults` option do not apply to this raw Node WebSocket observer. Actual outgoing
commands, target identity and the never-native sentinel provide the discriminating controls.

Frame capture now passes its real masked and unmasked viewport, magnified and target checks.
A later form case exposed a real correctness blocker: when page input makes a second prepared
field read-only, its value was appended to the first field. A rejected injected Promise could
produce no usable result while the worker continued. One shared injected error wrapper and one
result validator now protect both document-scoped and frame fallback paths. Seven regressions
passed: first effects remain exact and uncertain, second fields remain unchanged, preparation
failures have no effect, and missing or rejected responses never cause another insertion.
The full extension suite passed 349 tests. The final real frame journey passed all 69 checks,
including the exact first partial edit, unchanged later field and uncertain non-repeatable receipt.
Its earlier failure remains recorded; it was not waived.

## Final evidence and independent execution

The full suite used fresh siblings in `.target-quiet-trial/debug`, both optional compatibility
and native-installation lanes, local synthetic pages and exact source hashes. It passed 26 of
28 gates. The two failures were new fixture expectations for a future invocation after Stop:
the existing typed contract returns `blocked` with `session_ended`, rather than `cancelled`.
Both tests now require that exact status and reason, no effect, preserved values and no replay.
Both complete affected journeys passed after this test-only correction. Production source did
not change after the full suite. No failed result was deleted or converted into a pass.

All required commit gates passed: formatting, strict all-target workspace Clippy, 588 Rust
tests, `npm test` with 349 extension tests, and syntax checks on 35 changed JavaScript files.
The full suite also passed packaging, policy grammar, 18 recipe cases, process routing,
continuity, provenance, both CLI edges, desktop Workbench lifecycle, browser relay reconnect,
script/frame/history browser journeys, legacy refusal and actual historical custody/restart.
Current quiet acceptance passed 14 observed checks. Real installed native messaging passed
all five journey stages, including actual production WebView2 Show tab, Pause, Resume, Stop,
post-Stop reveal and no replay. Receipts were compared with physical tab/window state, retained
trusted input and debugger custody. Eight controlled debuggers detached in the quiet Stop check.

The installed profile and unique native registration were removed. Existing native registrations
and manifest hashes remained structurally identical. Root-owned processes exited; the user's
live release service and connector were preserved. No live-test owner remains, and browser
control is free for independent execution and the requested UX observations.

The consolidated local manifest is `.tmp/quiet-trial-evidence/final-verification.json`.
Its source fingerprint is
`7cb5c47b9eb664c33b597e6a343cf9e324edaf7242f2bc7fa2ada42a5f167268`.
Its production-source fingerprint, excluding tests, is
`3ab1e07f1f5e41f33228c9a871a0815fe198d0ef51752fb7468843071af345fe`.
The complete supplemental journey source fingerprint is
`d7f0b55d289dd82547b0ab29d051c00e03c84fee019ee63912c6aab69015b484`;
the native-installed before/after fingerprints match.

| Executable under `.target-quiet-trial/debug` | SHA-256 |
| --- | --- |
| `ghostlight.exe` | `e4f1bd84642124f22297c5d7062728171cc5826e3e8bfd623df9552dec007a89` |
| `ghostlight-mcp-connector.exe` | `290d50fbd6defb82b955acf49e980b0732ce5ce6511187a6ca15551d18c79451` |
| `ghostlight-browser-connector.exe` | `b82b41b5a7811a0d7718654d4f865ac55003d96805ce6bc3c3d1c1ff059e9be5` |

Run from the trial checkout. Each installed run creates its own disposable profile and unique
registration; the recorded profile has been cleaned up. No runtime override applies to the
installed native journey.

```powershell
$env:GHOSTLIGHT_BIN_DIR = Join-Path $PWD '.target-quiet-trial/debug'
$env:GHOSTLIGHT_TEST_BROWSER = Join-Path $PWD '.tmp/chrome-testing/chrome-win64/chrome.exe'
node tests/quiet-browser-journey.mjs
node tests/quiet-native-installed-journey.mjs
node tests/quiet-browser-journey.mjs --legacy
node tests/quiet-browser-journey.mjs --legacy-custody
```

The complete gate command was:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD '.target-quiet-trial'
$env:GHOSTLIGHT_TEST_BROWSER = Join-Path $PWD '.tmp/chrome-testing/chrome-win64/chrome.exe'
$env:GHOSTLIGHT_TEST_QUIET_COMPATIBILITY = '1'
$env:GHOSTLIGHT_TEST_NATIVE_INSTALLATION = '1'
$env:GHOSTLIGHT_RECIPE_SH = 'C:\Tools\Git\usr\bin\sh.exe'
$env:PATH = (Join-Path $PWD '.tmp/recipe-prerequisites') + [IO.Path]::PathSeparator +
  'C:\Tools\Git\usr\bin' + [IO.Path]::PathSeparator + $env:PATH
node tests/hardening-suite.mjs --lane=all
```

The compiled native recipe fixture and local jq are test prerequisites, not persistent PATH
changes. The manifest records source, executable, evidence JSON and screenshot hashes. The
logged focus control can be reproduced locally with `node .tmp/focus-origin-control.mjs` after
setting the same binary/browser environment. Its diagnostic pass is not product acceptance.

Three relevant production Workbench screenshots were saved from the successful installed run:

| State | Local artifact suffix after `.tmp/quiet-native-installed-2026-10-01T01-30-41-124Z-29976` |
| --- | --- |
| Paused | `-paused.png` |
| Show tab while paused | `-paused-show-tab.png` |
| Session ended with post-Stop reveal | `-workbench.png` |

The screenshot hashes and additional human/initial-load captures are in the installed JSON
and consolidated manifest. The file named `-active-effective-attention.png` was captured before
connection hydration and visibly says Not connected; it is initial-load evidence, not a Ready
state screenshot. Earlier static UI captures predate refined attention copy and are historical
evidence, not screenshots of the final candidate.

Immediately before and after the quiet flow, the same external OS foreground process and
window remained in control. Chrome's internal focus flags and synthetic human input retention
were measured separately. This is not physical typing in a foreground Chrome window.

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
