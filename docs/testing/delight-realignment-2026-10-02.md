# Quiet confident completion -- candidate evidence

Date: 2026-10-02. Status: Historical first source freeze `66213c61`; native acceptance rejected.
The [native review](quiet-success-native-review-2026-10-02.md) found retained observation custody
and a hidden composed wait purpose. The [later correction](quiet-success-correction-2026-10-02.md)
is the current source, identity and acceptance handoff. Nothing was installed, pushed or published.

The observation recovery checks below used short budgets and polled for scope release before
the next read. They prove eventual recovery and outcome truth, not immediate recovery. The native
20000 ms probe exposed that gap. Do not use this historical report as current native acceptance.

## Structural-review checkpoint

The implementation scope is stable. Read the current tracked diff, plus the new
`crates/orchestrator/src/language/progress.rs`, `tests/delight-journey.mjs`, and ADR-0192.
Work changes are in `work/{mod,forms,resolution}.rs`; frozen cause/projection changes are in
`language/{audit,outcome,resolution,history}`, live purpose in `language/progress.rs`, and
caller guidance in `language/catalog.rs`. Composition cause narrowing is in `work/composition.rs`;
the existing operation projection is in `workbench/mod.rs`. The UI
changes are in `ui/{app,index,styles}` and `ui/lib/{words,store,view}`. No connector or adapter
production code changes. No new task/lifecycle system. The guardian card is intact.

The robust happy journey already passed on the baseline; it remains a two-call discovery/flow
path. No new happy-path completion-rate or call reduction is claimed. Existing inspection handles
already provide the direct precise path. Architecture corrections remove the extra ambiguity
mechanism and classify unavailable reads using the exact command's existing effect evidence.
Native UI ownership remains with the parent until explicit handover. The installed PID/hash are
preserved. The current patch, new files and `.tmp/delight-review-manifest.json` identify the
source for review; `.tmp/delight-review-tracked.patch` captures the tracked implementation diff.

The approved slice is a complete synthetic "save a draft while I keep working" journey through
CLI and MCP. Its measures are first-attempt completion, unnecessary calls, human interruptions,
reliable confirmation, and cleanup. Ambiguity, slow pages, and cancellation challenge the same
journey. This report will distinguish executable, Chromium component, and independent native
acceptance evidence. Test totals alone do not establish easier successful work.

## Starting custody

- Branch: `codex/evaluation-cancel-lease-fence`.
- Checkout: `8bd62ecfe24bf14383a3e55d92a0b0afbd6d8ac7`.
- Installed implementation reported by the independent review: `f7f00f071b79b0ff42b51cacd116287953ab945a`.
- Pre-existing work: the independent delight report and its uncommitted STATUS entry. Preserve both.
- No production runtime replacement, persistent access/security edit, public change, Tangent
  access, or real-tab cleanup is authorized. Isolated candidate testing is authorized.

## Baseline diagnosis and bounded proposal

The existing draft form, semantic selectors, flow, background attention, exact resource custody,
effect evidence, and canonical completion are the starting primitives. No task engine is needed.

| Cause in the live source | Proposed correction at its owner |
| --- | --- |
| `work/mod.rs::browser_failure` gives pre-dispatch cancellation a cancelled status but a BrowserStopped refusal. | Keep cancellation and connection loss distinct in the existing resolution. |
| `work/mod.rs::verify_action` caps a short declared check at two seconds. Slow application completion can exceed that check. | Keep the short check. Use an explicit application-condition wait and read in the same flow for slow completion; compare browser observation with independent fixture persistence. |
| `work/forms.rs::perform_wait` repeats visual settlement in three branches and discards errors. Its optional-target helper does not project the resolved tab. | Use one bounded wait completion and settlement path, preserving cancellation and actual readiness. Publish content-free condition/budget and exact owned tab through existing work progress. |
| `ui/lib/store.js::settled` trims snapshot history to the 200-row feed bound, while snapshots restore 500. Clear view hides only rows presently displayed. | Separate retained history from displayed rows, hide all completed retained entries when clearing, and expose the view's scope and retained-history route. |
| Language's human outcome classifier treats the preserve-tabs interlock as ordinary failure. | Keep the blocked/no-effect facts and both close gates, with a calm preserved-tab presentation and exact manual guidance. |
| Selector recovery asks for role/exact even when already supplied. | Keep safe refusal and one existing inspection route; use precise current handles, never choose ambiguous controls. |
| Workspace summaries label owner-retained CLI workspaces as connected sessions despite zero live connections. | Distinguish connected work from retained tab custody in the existing projection; preserve caller continuity. |

CLI and MCP stay thin. The complete guardian About card, artwork, palette and motion remain product
identity. Displaced completion and display paths are removed, not kept as fallback alternatives.
The parent's architecture guidance confirms these owners. Empty continuity while a CLI caller
is live is legitimate, not a demonstrated leak. Do not remove it for presentation convenience.
Payload compression, onboarding and broader redesign stay outside this slice. Native UI review
is coordinated with the parent; component journeys do not substitute for that acceptance.

## Journey evidence

The runner is [tests/delight-journey.mjs](../../tests/delight-journey.mjs). It uses the real authority,
CLI and MCP connector, the shipped MV3 worker/page runtime, an isolated Chromium profile and a
loopback native-port shim. Its local form accepts only trusted input and re-renders the controlled
values. A separate HTTP server counts persisted saves and stores their fields. Browser command
receipts, page values, server persistence, human-fixture focus/ownership, resource state and audit
are recorded separately.

Raw final runs:

- [Baseline](../../.tmp/delight-baseline-comparison-2026-10-02.json), frozen source at `8bd62ecf`
  (the implementation at `f7f00f07`, before this slice).
- [Candidate](../../.tmp/delight-candidate-comparison-2026-10-02.json), the first frozen source.

| Journey or challenge | Baseline | Candidate |
| --- | --- | --- |
| Ordinary one-flow open/fill/click with a short check, CLI and MCP | Completes in one client call; one persisted save | Same |
| Save completes after 3.5 seconds, using only short click `expect` | Partial result; one extra wait; no replay | Same; short `expect` was deliberately retained |
| Bounded discovery, then one fill/Save/fresh-condition wait/read flow | All eight CLI/MCP cases complete first attempt in two client calls | Same, with exact owned wait reveal/purpose through the shared projection |
| Controls appear after 2.4 seconds | One selector-present wait, then one discovery; no repeated targeting | Same |
| Old generic "Draft saved" marker before server save | Short check can succeed while independent server count is zero | Same limitation, explicitly documented; fresh ID-specific condition completes only after server save |
| Two enabled controls have the same exact role/name | Failed, no effect, zero clicks | Same; existing inspection guidance, no automatic choice |
| Cancel a local delay before physical dispatch | Cancelled/none but says the browser disconnected | Cancelled/none, says cancellation won before the next command; next read succeeds |
| Cancel a dispatched read-only observation | Unknown/unknown; says the action's effects were not confirmed | Cancelled/none; honest unavailable observation; next read succeeds |
| Cancel settlement after the requested text appeared | Unknown/unknown with a text-never-appeared sentence | Cancelled/none; preserves `condition_satisfied: true`, unavailable settlement and unknown readiness |
| Cancel after acknowledged field fill, before Save | Cancelled/partial, draft retained, zero saves/clicks, unsafe to replay | Same truth; exact cancellation cause retained |
| Human Pause before the next flow effect | Blocked/partial; filled draft retained; Resume does not replay Save | Same |
| Cancel or deadline after script mutation, then observe | Unknown/unsafe; next read succeeds; page code later reaches a second effect | Same; no stopped-JavaScript claim |
| Preserve-tabs rejects close | Blocked/none; audit presentation "Could not complete"/failed | Same gates and tab; presentation "Tab preserved"/controlled; no repeated native popup |
| Human keeps editing during a slow draft save | Trusted CDP text input reaches the owned human form; focus/active window/ownership stay fixed | Same through both CLI and MCP |

Each successful draft scenario persisted exactly one save with the intended title and draft.
The explicit-condition journey uses one discovery and no recovery calls or replayed mutations.
Both runs have zero interruptions in the measured fixture focus/ownership state. This is not a
measurement of a real person's attention or OS keyboard focus. Identical enabled controls remain
unresolved. Existing inspection supplies current precise handles for controls the caller has
actually identified.

The recommended caller sequence is ordinary tool composition:

1. Open an owned tab, wait for a late control if needed, and inspect once in a bounded flow.
2. Use the current unique handles in one flow: fill fields, click Save once, wait for the
   application's fresh confirmation within the original deadline, then read.

For the fixture, confirmation includes its unique draft ID and appears after server commit.
Ghostlight only reports browser observation. Arbitrary sites need an application condition that
actually establishes their intended result; no generic confirmation subsystem was added.

## Activity and settled truth

Source diagnosis found three duplicated settlement branches that discarded errors and used old
readiness. They are replaced by one validated condition/settlement path. Fault tests establish
that primary Interactive plus settlement Complete produces Complete, and cancellation or cleanup
failure during settlement preserves the actual refusal with `condition_satisfied` and phase facts.
A local delay that exceeds its deadline cannot invent uncertain mutation. Exact resolved-tab and
purpose updates occur while the delay is running, not only after completion. An unavailable
observation does not imply a mutation. Live progress and audit contain no raw condition text.

The store now holds 500 retained groups independently of the 200 displayed entries. Event and
snapshot retained counts/bounds agree for the tested cache cases. Completed-only history retains
its tested order; general live hero ordering equivalence is not claimed. Clear view hides all
500 completed groups, retains live operations,
and survives resync. Its count reports the groups actually hidden. Show retained activity restores
the display without deleting audit, dropping new live work, or reviving completed operations.
An empty cleared view says "No activity in this view." Connected work, retained CLI tabs and
dormant empty continuity have distinct presentation. No caller/workspace lifetime was changed.

Owned bundled-UI renders and interactions passed at 1280 and 720 pixels:

- [Wait, exact tool and scoped activity at 720](../../.tmp/h4-delight-wait-history-720.png).
- [Same view at 1280](../../.tmp/h4-delight-wait-history-1280.png).
- [Clear view with retained-history route](../../.tmp/h4-delight-cleared-720.png).
- [Complete guardian card at 1280](../../.tmp/h4-delight-guardian-1280.png) and
  [720](../../.tmp/h4-delight-guardian-720.png).

The guardian image is unchanged in both runs:
`153e65ae92af61a7cd2dcbe38c59e6875287a9e3f0208fb4e73f781292327a67`.
The card text, art, palette and motion remain. Only adjacent admitted-workspace and retained-history
facts were clarified. These synthetic renders do not prove installed Tauri/native keyboard UX.

## Independent architecture corrections

The added ambiguity result contract, cap, handle registrations and mechanism-specific tests were
removed after review. Ordinary inspection already supplies generation-bound handles for precise
action. The deliberately broad duplicate-selector recovery fixture is retired; no improvement
from that removed mechanism is retained as a candidate claim. Safe ambiguity refusal and useful
existing-inspection guidance remain. The robust happy journey has baseline parity.

Lost observation receipts need exact command evidence, not a global after-dispatch uncertainty
rule. The revised command/evidence seam keeps read-only cancellation/deadline/disconnection
effect-free while preserving the unavailable observation and actual cause. Fault tests cover
all three causes in primary and settlement reads, alone and after an applied prefix. An uncertain
script followed by an unavailable read remains uncertain. Flow replay restrictions remain.

The matched Chromium comparison cancels only while a finite-animation observation is still in
flight, after its browser dispatch and document scope are recorded. Both next reads succeed, with
zero clicks/saves and no residual evaluation, scope, watcher or request.
[Correction comparison](../../.tmp/delight-correction-comparison-2026-10-02.json) carries the exact
baseline/candidate outcomes. The baseline reports uncertain mutation for a read and loses the
settlement cause behind a text-never-appeared sentence; the candidate reports cancelled/none,
unknown readiness and the unavailable observation without claiming the whole wait completed.

Two inherited issues are outside this slice: the newest of two live operations can settle while
the earlier still runs, and resync can reorder the hero; document/tree inspection can store
context internally but project only metadata/counts. The fixed count/cache bounds do not establish
general ordering equivalence. A focused [two-operation diagnostic](../../.tmp/delight-inherited-ordering-2026-10-02.json)
reproduced the hero mismatch on both sources. Investigate the missing inspection context at its existing owner;
do not add a flat list as a substitute.

## Candidate identity

All final executable lanes used `.target-delight-candidate/debug`; none resolved stale default
executables. SHA256:

| Image | Baseline | Candidate |
| --- | --- | --- |
| Authority | `eabbeaec9afce3010b51d449454a5183eedaf5d93a7d023376b21dfb400e102c` | `66213c6113a32e4524ca93c4dd1a5d31c88a426122c7a1fcd8b1e64d794a9df6` |
| MCP connector | `2664edba1fc932f43142b96ea603490d9a91f56a3decd83fabd3c143a2493ba0` | `ad8efa84aac3e269327f502a862e505b588797c5a73f973a558236640aebf8f0` |
| Browser connector | `6d0cd631c5d6c285d246f9f2f15d9f9bf1215c471e8ddd90a4f9c132ef72c1b0` | `57a0a8c44dd0ddb262554ae4c9648bdc0b07f2d1803401592161f79849dd0096` |

The connector sources/contracts did not change. Their rebuilt debug image identities differ
because they are linked from distinct isolated build directories. This is not a connector rollout.
Installed `target/release/ghostlight.exe` remains
`bade8ca680362fb0a5e16d523f3381d41c8319e07a815c53458d37f8f6750945`, running as PID 8332.

The parent-owned native preview can use `.tmp/delight-native-review.ps1`. The script was parsed
and its PowerShell child-only environment support checked. It verifies the authority hash above,
uses unique isolated paths, and has not been executed. Do not install or redirect the existing
adapter. Source files are hashed by `.tmp/delight-review-manifest.json`; STATUS is excluded
because it includes the owner's pre-existing review entry.

## Tests, cleanup and limitations

Final checks passed:

- `cargo fmt --check`, strict workspace/all-target clippy, and `cargo test --workspace`, using
  the isolated candidate target. [Rust log](../../.tmp/delight-rust-tests.log),
  [clippy log](../../.tmp/delight-clippy.log).
- Extension `npm test`: 393 passing, none failed. [Log](../../.tmp/delight-extension-tests.log).
- `node --check` for all changed UI and journey JavaScript; changed-file ASCII and diff whitespace.
- Workbench surface and real bundled Chromium rendering/interaction, including Clear/restore,
  settled/live updates, Space/Enter controls in the component fixture, reveal without Resume,
  recovery details and both widths. A screenshot-only timeout during the refresh passed on one
  fresh owned-profile retry; no product code changed for it. [Surface log](../../.tmp/delight-workbench-tests.log),
  [browser log](../../.tmp/delight-workbench-browser.log).
- Real process and CLI lanes. [Process log](../../.tmp/delight-process-journey.log),
  [CLI log](../../.tmp/delight-cli-journey.log). Two CLI assertions omitted already-existing
  `Next:` guidance; the first failed identically on frozen baseline and candidate. The corrected
  lane passed on both. Its governed phase now inherits the isolated native-host environment.
- Restart/predecessor/optional-cause checks preserve every pre-existing audit byte, with only the
  deliberately unsupported required record counted unreadable. This compares the frozen debug
  baseline reader with the candidate, not an installed predecessor package.
  [Compatibility log](../../.tmp/delight-history-compatibility.log) prints its evidence directory.
- Final baseline and candidate draft/challenge runners, including continuing trusted human-fixture
  input, independent persistence, dispatched read and prefix/uncertain cancellation, Pause and
  exact cleanup. The process lane's old read-cancellation assertion was corrected to the precise
  cancelled/none contract; its mutating unknown-effect assertions remain.

Cleanup and deletion accounting:

- Each final draft runner exited its 15 top-level owned child processes, closed the owned
  Chromium process tree, removed its profile/shim/policy/native-host/diagnostic scratch, and
  deleted its exact runtime record and lock. Neither final run left an authority or browser alive.
- After cancelled/paused work, evaluation records, document scope, navigation watchers and active
  requests were zero. The 17/17 attached managed tabs were intentional retained workspace custody,
  not outstanding command leases. They disappeared with the owned browser/profile at teardown.
  Released evaluation custody did not stop page code: the independent page counter reached two.
- Process/CLI/compatibility fixtures use isolated runtime/native-host paths. The CLI lane checks
  byte-identical machine registration before/after. Final process inspection found no remaining
  `.target-delight-*` authority or owned delight/history Chromium process.
- Source removal is deliberate: three duplicated wait-settlement branches and their error-swallowing
  fallback are gone; the competing UI queue-count writer/export is gone; the 200-entry history
  truncation, displayed-only Clear, exhausted selector-refinement advice and cancellation-to-
  disconnection fallback are displaced. There is one current path for each behavior.
- No inherited source files, historical documents, product assets, tools, public contracts or
  tests were discarded. The temporary ambiguity contract, cap, registrations and dedicated
  test/recovery fixture were fully removed after architecture review.
  The 24-tool catalog, caller continuity, final target/policy checks and uncertain-effect fences
  remain. The pre-existing independent report and its STATUS content are preserved.
- Isolated build directories and ignored evidence/logs/screenshots remain intentionally for review.
  The initial diagnostic prefix probe recorded cancellation before the fill receipt was acknowledged;
  it was corrected to wait for authority acknowledgement, then both final runs passed. That probe
  is separate from the final evidence. Fixture startup/debug iterations cleaned their owned scratch. One observation probe finished
  before cancellation; it was replaced by finite animation plus a pending-request assertion.
  One candidate probe read CLI stdout before stream completion; it now waits for `close` and
  requires a receipt. These diagnostic runs are retained separately from final evidence.

Remaining acceptance is independent architecture review and installed/live UX red-team review,
including real OS foreground/keyboard ownership, ordinary client rendering, native-port discovery,
and actual operator attention. The component runner's native-port shim and CDP input are explicit
limits. There is no implementation blocker and no satisfaction score or user-research claim.
No deployment or persistent access/security change, real-tab cleanup, Tangent access, push,
publication, model-private memory update, or DOCX was performed.

The shell sandbox could not apply its deny-read ACLs. Authorized inspections/builds used the
automatically approved shell fallback; no approval rejection or scope expansion occurred.
