# H2b: Accurate progress and contextual recovery

Status: implemented and verified locally; deployed locally on 2026-09-07, not published.
Owner decision: 2026-09-06. See the [deployment record](../../STATUS.md#local-deployment-2026-09-07).

## Accepted behavior

Completed means succeeded. Keep successful, failed, blocked, cancelled, attention-required,
uncertain, unable-to-start, and never-run steps distinguishable. Continue permits later independent
work; it cannot turn an unsuccessful step into success. Known partial progress stays known.
Preserve previously applied effects when later effects are uncertain. Recovery respects existing
progress and suggests observation when necessary, rather than replaying the whole composition.

Keep Stop/Continue and the existing terminal status/effect vocabulary. No new policy setting.
Human controls and invocation cancellation/deadlines remain stronger than Continue. Preserve the
pinned human-control directives. H4 owns child audit rows and expandable history. Resumption UI and
automatic reconstruction of remaining work are future opportunities, not part of this cycle.

## Bounded implementation

One logical commit replaces independent flow/sequence counting with one outcome accumulator and
one language-owned composition account. Keep one parent lease and authority snapshot. Each listed
step has bounded status/effect metadata even if its result payload exceeds the return budget.
Unreached steps get an explicit not-run row without a fabricated child invocation. Runtime
reference/decoding failures have an unable-to-start reason and send no browser work.

The existing `completed` and `completed_steps` fields now count successes. Add a shared `progress`
account carrying disjoint status counts, effect counts, the relevant problem position/cause, and
whether execution stopped. The same typed, payload-free account may enter H1 audit. No raw child
results, ids, errors, or target handles enter that projection.

Aggregate status uses the existing vocabulary: unknown effects remain unknown; otherwise attention,
cancellation, policy blocks, and failures remain non-success. Successful completion requires every
step to succeed. An applied effect plus incomplete work is partial; a partial child remains partial.
Safe repetition requires a fully successful, effect-free composition whose children all permit it.

## Acceptance

- Show regressions failing before the correction for Continue success/counts and known partial work.
- Cover stop/continue after policy, decoding, and reference failures; later independent work;
  a failed final step; and failure-free Read compositions.
- Cover prior applied/partial effects followed by unknown effects, plus cancellation/deadline
  boundaries and retained human-control directives.
- Cover sequence/flow agreement, result-budget omission, safe metadata in actual JSONL, and MCP
  error signaling. Keep client details separate from H1 audit metadata.
- Run the repository commit gates and a fresh-build process journey. Record physical-lane limits.

## Implementation and evidence (2026-09-06)

One logical change: `fix(flow): preserve accurate composition progress and recovery`. Use Git for
its commit hash. The implementation replaces the two independent aggregators and old FlowRan /
SequenceRan sentences. It adds no input flag, policy setting, relay protocol, or extension logic.
The parent retains the same lease and immutable authority snapshot. The first child policy denial
cannot be overwritten by a later allowed child in the parent audit decision.

The progress account contains no strings except closed enum spellings. Live rows can retain
permitted payloads and decoding details. H1's audit projection receives the typed account only,
under `composition`. Actual after-dispatch connection loss and cancellation now have distinct
closed refusal metadata, so composition never infers these causes from arbitrary error text.
Known partial work receives observation guidance that preserves confirmed changes. Human pause
and stop preserve their pinned directives and offer no action to repeat the work.

Verification:

- Initial regression run: the refused-child test failed because `completed` was 2 instead of 1;
  the runtime-input test failed because the never-run row was missing. This red run stopped at
  those first assertions; the later Continue assertions were exercised after the correction.
- All 378 orchestrator library tests pass. New executor cases cover policy denial followed by
  independent success, runtime argument/reference failures under Stop/Continue, failure of a final
  read, and fully successful repeat-safe Reads. Direct sequence and flow paths both preserve
  prior applied effects on failure or connection loss and refuse dispatch of the later step.
- A result exceeding the 100,000-byte envelope budget omits child payloads while retaining all
  step status/effect/repeat metadata. A later reference still resolves against that volatile result.
- The shared accumulator covers prior applied/partial effects followed by uncertainty, attention,
  cancellation, and known failures. Boundary cancellation/deadlines retain unexecuted counts and
  prior effects without fabricating child failures. Human pause/stop directives survive aggregation.
- Continue stops on attention, cancellation before/after dispatch, and deadlines before/after
  dispatch. These tests preserve the current runtime contract; they do not establish H5 timing.
- All 455 workspace Rust tests, all 183 extension tests, formatting, workspace Clippy with warnings
  denied, and JavaScript syntax checks pass. No extension JavaScript changed.
- Repository integrity passes for 918 tracked files, local links, ASCII, version alignment,
  permission justifications, and the capability matrix. Changed-file ASCII and diff checks pass.
- Fresh `cargo build --workspace --target-dir .target-ghostlight-1.0` and process journey with
  `GHOSTLIGHT_BIN_DIR=.target-ghostlight-1.0/debug` pass. Actual MCP error flags distinguish mixed
  Continue, policy block, and unable-to-start results. JSONL `composition` equals result `progress`,
  successful counts equal the observed measurement, and private sentinels remain absent from audit.

This is Windows executor and real-process evidence with a synthetic native adapter. No installed
MV3/native-host or live Chromium timing lane was run for H2b. H4 child audit records, expanded
history, and resume UI remain unimplemented; H5 scope/timing and other undecided packages retain
their ideation gates. Existing audit files were not rewritten. Published versions are unchanged.
