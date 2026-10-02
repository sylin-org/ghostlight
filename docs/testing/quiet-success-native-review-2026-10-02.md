# Independent quiet-success native review -- 2026-10-02

Paths under `.tmp/` identify local verification artifacts, not distributed files.

**Verdict: REJECT complete native UX acceptance.** Ordinary draft work succeeds quietly and
effect truth is useful. A cancelled native observation still prevents the next read until its
original budget expires. Correct cancellation wording does not repair that journey.

This is an independent expert review of owned synthetic work, not user research, a satisfaction
score, release approval, or acceptance of the installed user's everyday Chrome session.

## Candidate and actual execution

Base: `8bd62ecfe24bf14383a3e55d92a0b0afbd6d8ac7`, with the frozen uncommitted patch.
Authority SHA256: `66213c6113a32e4524ca93c4dd1a5d31c88a426122c7a1fcd8b1e64d794a9df6`.
Manifest SHA256: `400fe2656abff88cc4bea1bd48852a80e297dd62365988f744a917a4bc3e3db9`.
All 32 manifest files matched before and after review.

The actual candidate authority was PID 36832. The real Tauri HWND was 7995838, verified by
executable, PID, title and window class. Screenshots use window-only native PrintWindow capture.
They are not component renders. The unmodified development extension in an owned visible Chrome
for Testing profile used the existing native registration. Its real native host, PID 23148,
inherited the process-local runtime override and connected to the candidate. The candidate MCP
connector served a normal stdio review client. No native-port shim was used.

The prepared launcher was inspected and copied under ignored review state to add an explicit
child-only WebView profile. A preliminary empty preview was stopped before browser work; its
default profile was not acceptance evidence. Runtime, policy, controls, audit, diagnostics,
Chrome and WebView profiles were isolated for the acceptance runs. No installation or registration
was replaced. Final identities (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/50-final-identities.json`)
and process ancestry (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/50-before-cleanup-processes.json`)
preserve custody.

## Findings

| Finding | Observed evidence and impact |
| --- | --- |
| High: cancelled observation retains document custody | A dispatched visual observation had an exact document scope and a pending MCP request. Cancellation returned `cancelled/none` in 109 ms, but scope remained held even though active requests were zero. The immediate read failed in 28 ms with `operation_cleanup_required`. Scope released 19,198 ms after cancellation, near the original 20-second observation budget; only then did reading succeed. Its recovery text suggested global End/Start if pending. This materially blocks quiet recovery. Timed counterproof (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/native-observation-cleanup-repro.json`), native aftermath (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/36-native-observation-recovery.png`). |
| Medium: composed save hides its running wait purpose | During the real 12-second application save, the collapsed native hero said only `Completed 2 of 4 steps.` It exposed Show tab and elapsed time, but no wait purpose, phase or budget. Direct waits did expose purpose and budget. This does not prevent saving, but weakens the scoped promise of comprehensible sustained waiting. Save wait (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/13-cli-native-confirmed-wait.png`), direct duration wait (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/39-native-duration-purpose.png`). |
| Inherited: document inspection projects no tree | Scoped document inspection reported eight nodes and a snapshot but returned no tree or context. Existing `all` inspection supplied named form/section/control context and exact handles for this fixture, so the gap did not block its save. It remains material for callers needing the promised subtree. Actual result (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/05-document-context.json`). |

The native cancellation failure is observed on this candidate. I did not run a matched native
baseline, so I do not label it a new regression. The changed wording is truthful; the recovery
mechanism still needs proof. The separately documented inherited two-operation hero/resync issue
was not rerun natively. It does not explain this sequential save's success, and concurrent
orientation remains unaccepted.

Acceptance should require cancelling a long-budget native observation and immediately reading
the same tab successfully, with exact custody retired. A short original wait budget can conceal
the hold by expiring before a delayed follow-up check. Do not replace this proof with wording or
zero active-request counts.

## Journeys that worked

- MCP saved one draft after a 6.5-second application delay. CLI saved one after 12 seconds, using
  one discovery flow and one fill/Save/fresh-confirmation/read flow. Independent server requests,
  committed fields, retained page model and click counters agreed: one request, one save and one
  real Save click per fixture. Dispatch alone was not counted as persistence. The baseline already
  supports two-call robust work; no speed or call reduction is claimed.
- Two enabled exact-name Save controls refused with no effect and zero save requests. The caller
  identified the launch-note control through existing `all` inspection and used its current exact
  handle; the rehearsal control was never activated. The challenged MCP path used four client calls,
  including ambiguity and document inspection, rather than a claimed two-call total.
- Cancellation before mutation returned `cancelled/none`. Cancellation after an acknowledged fill
  retained the draft, returned `cancelled/partial`, and never reached Save. An actual script mutation
  cancelled after dispatch remained `unknown/unknown`; immediate reading succeeded while its page
  counter was one, then page continuation reached two without replay. These cases did not duplicate
  the saved draft. Raw journey checks (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/independent-journeys.json`).
- Trusted synthetic human editing continued. The original unsaved text, added text, active tab,
  focused editable and unowned status survived. This measures fixture continuity, not a real
  person's attention or physical keyboard focus.
- Protection remained enforced and appeared calm in the mixed native activity list. Exact tool
  names and readable client identity remained useful. CLI tabs were explicitly retained rather than
  falsely counted as connected clients. Mixed native history (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/38-native-protection-history.png`).
- Native Show tab selected the exact owned fixture without Resume or mutation replay. Clear view
  changed 21 shown / 21 retained groups to 0 / 21; restore returned 21 / 21, with unchanged audit bytes.
  Presentation results (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/native-presentation-final.json`),
  cleared view (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/47-native-cleared.png`).
- The complete Sylin guardian card, artwork, cyan divider/diamond, version medallion, abilities and
  flavor line remain intact in the actual native About view.
  Guardian screenshot (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/34-sylin-guardian.png`).

## Limits, diagnostics and cleanup

The Computer Use skill was read; its required node_repl runtime was unavailable. Native Windows
accessibility supplied guarded invocation and capture. Exact foreground ownership was established
for keyboard probes, but the control-focus guard failed. **Zero keys were sent.** Physical Enter,
activity expansion and full OS foreground transfer to Chrome remain unproven. Logical exact-tab
selection is confirmed. Named-client rendering, native narrow layouts, 500-group behavior and the
post-condition settlement-cancellation branch are not accepted by this pass.

Bounded harness diagnostics are preserved: the first capture guard counted a legitimate CLI child
sharing the authority image path; it was corrected by pinning the verified authority PID. That
diagnostic fixture saved once and was not replayed. A human-text assertion wrongly required the
old text to remain contiguous despite insertion at its caret; a control edit proved no loss. One
textual observation probe reached its own deadline without exercising the desired pending visual
observation. A presentation probe ran while About was selected; final checks explicitly restored
At a glance. These are not candidate defects or acceptance passes.

Before teardown, evaluation records, exact document scope, navigation watchers and active requests
were zero. Three debugger attachments were intentional retained tab custody. All owned tabs closed
with the owned Chrome profile; its host, MCP client, fixture driver, candidate and WebView helpers
exited. Chrome/WebView profiles and both owned runtime/lock/startup sets were removed. Evidence stays.
Installed authority PID 8332 and browser host PID 35824 retain their original creation times; the
installed authority and native manifest hashes are unchanged.
Cleanup proof (local evidence: `../../.tmp/delight-native-review-244f883dca6643909dc7a8e894fa250a/51-cleanup.json`).

No product edits, deployment, push, broad reset, persistent grants/security changes, Tangent access,
real-tab closure, real draft/history deletion or public action occurred. Only this report and the
repository status handoff were added outside the frozen source manifest.
