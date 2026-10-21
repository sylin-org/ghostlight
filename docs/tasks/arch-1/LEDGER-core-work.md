# arch-1: core-work ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Files: `work/**` and
`workspace/**`. Machine: test-03 (Alpine). A slice's specification is its finding in
[sweep-2026-10-20/work.md](sweep-2026-10-20/work.md), read with its skeptic's corrections, or its
entry in [ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`,
`landed` (with commits), `blocked` (with the reason).

## RESUME HERE

W-D2, W-D3, C19c, C10c, C12b-2 and WORK-12 have landed, with WORK-2b's workspace half and
WORK-7's lease half. W-D1 moved to core-language (L-D5); D52(c) went to test-02. Next: C16,
whose cross-lane holds are granted (test-02's five adapter files, test-04's `workbench/tests.rs`,
the coordinator's `browser/mod.rs` and `service/mod.rs`). A slice's hash is
filled in by the next slice's commit, since a rebase before the push changes it.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| W-D1 | fix(work): an inline upload of exactly 5,000,000 bytes is accepted | - | WORK-1's note | moved to LEDGER-core-language.md (L-D5) |
| W-D2 | fix(work): a flow step whose tab close the policy refuses shows the kept-tab notice | - | WORK-8's notice half | landed (ecf30a64) |
| W-D3 | fix(work): a selector wait gives the readiness fact the other waits give | - | WORK-13; a deliberate model-visible change | landed (f6d9fdc3) |
| C19c | refactor(work): the completion path's leftovers go | - | ADR-0203 C19c, moved from core; WORK-7's `CompletionGate`; the service half is core-service's | landed (244875d3) |
| C10c | refactor(work): the executor reads one publication | P1, P5 | ADR-0203 C10c, moved from core | landed (616bfffa) |
| C16 | refactor(work): which recording a workspace holds is a workspace fact | - | ADR-0203 C16, moved from core; removes SERV-9's "reaped" | landed (c43e2e20 and the port commit) |
| C12b-2 | refactor(work): the typed refusal builds its result (the work half) | C12b-1 | ADR-0203 C12b; WORK-4, WORK-2 and WORK-3's refusal sites | landed (c1603722) |
| WORK-3 | refactor(work): the typed outcome builds its result too | C12b-2 | WORK-3; four sites keep their overrides | todo |
| WORK-1 | refactor(work): the upload ceiling has one home | W-D1 | WORK-1; the screenshot cap reads the adapter's `MAX_BASE64_CHARS` | todo |
| WORK-10 | refactor(work): timings and limits are named | - | WORK-10 | todo |
| WORK-7 | refactor(work): dead code goes | C19c | WORK-7; the two unused lease methods go | lease methods landed (6e38949a); `substitute_references`'s `Option` todo |
| WORK-9 | refactor(work): one name per contract | - | WORK-9; no persisted word changes | todo |
| WORK-13 | refactor(work): one finish for every wait | W-D3 | WORK-13 | todo |
| WORK-6 | refactor(work): `work/mod.rs` splits on its seams | WORK-3 | WORK-6; the moves land alone | todo |
| WORK-12 | refactor(workspace): its tests and its two books move out, and the lease helpers come in | - | WORK-12; the moves land alone | landed (8108ea82, 66a8842b, c65ec18d, dcb985e0) |
| WORK-5 | refactor(work): the closed vocabularies are matched as types | LANG-3 | WORK-5 | todo |
| WORK-2b | refactor(work): `HandleKind` at the mint sites, and the crate `lock` helper here | LANG-2, SERV-7 | LANG-2 and SERV-7's adoption | workspace half landed (ec6ae3fd); `work/mod.rs`'s stage locks ride with C10c |
| D52(c) | fix(work): `routing_refusal` passes the browser's cause through, and `capture_too_large` reaches `Refusal::CaptureTooLarge` | C12b-2, D52 | Decision 5.2's third part | taken by test-02 (109e30c3) |

## Notes

WORK-14 is not taken: browser events change leased workspaces through the store by design, and C4
recorded the choice. WORK-8's typed coverage is not taken either; ADR-0203's audit-projection
route stands. WORK-11 and WORK-15 are policy's (LEDGER-policy.md).

W-D1 moved to core-language as L-D5 on 2026-10-20. The refusal is intake's estimate in
`language/mod.rs` (`data.len() / 4 * 3`, padding included): 5,000,000 bytes are 6,666,668 base64
characters, which it counts as 5,000,001, before work runs. Work's own decoder counts exactly.
L-D5 has intake count through a public counter in `language/`; WORK-1 then points
`work/forms.rs`'s `base64_decoded_len` at that counter.

W-D2 reads the notice from the typed `Terminal` (ADR-0203 Decision 5.6): the decision's
`TabCloseDenied` gives the kept-by-policy notice and the `LocalInterlock` audit refusal the
kept-by-setting one, whatever the tool. A flow keeps its first refused step's decision, so a flow
stopped by a refused tab close now shows the kept-tab notice. The four-clause WorkBlocked test in
`complete_terminal` stays for C12b.

W-D3 adds `readiness` to the selector wait's facts, the value its envelope already carried and
the duration wait already reports. Regenerating the result goldens changed nothing, because no
result golden holds a wait, so the extended `waiting_on_a_semantic_selector_polls_the_live_page`
pins the new fact. The edit stays out of the condition matches C17 retypes.

C19c's work half deletes `CompletionGate`, its `CompletionError` and its test: the executor hands
its one `Terminal` to `complete_terminal` by value, so a second completion cannot be written
(Decision 4.4). Decision 4.7's comments on `Decision.allowed` and `ReasonCode::InvalidRequest`
say what is true: a policy refusal and two refusals no policy made (a call that did not match the
catalog, the person's hold or stop) set `allowed` false, while a cancellation or a call refused
during a quit keep it true. `governance/mod.rs` is the policy lane's, which is this machine's
next lane. The `DomainEvent` fields, `AuditRecorder::record` and the service lines are
core-service's C19c-s.

WORK-12's moves landed alone: the tests to `workspace/tests.rs`, `TargetBook` and its two bounds
to `workspace/targets.rs`, and the released-tab pool (with `reusable_site`, which only the claim
reads) to `workspace/released.rs`. `workspace/mod.rs` went from 4,820 lines to 2,353.

WORK-12's helpers: `WorkspaceLease::workspace_in` and `workspace_in_mut` replace the
lock-and-lookup preamble at 16 sites; `WorkspaceState::current` is the binding-on-this-document
check at the six sites that compare a selected tab's generation; `AggregateState::owned_elsewhere`
takes the five scans for another workspace holding a handle; and `resolve_view` holds what
`resolve_view_point` and `resolve_view_region` shared, the checks and the `SelectedView`. The three
generation checks that compare a record with its own tab keep their own refusals.

WORK-2b, the workspace half: `short_handle` takes a `HandleKind` (LANG-2, 01cc55e6), so the five
mint sites name their kind and read its prefix and `WORKSPACE_SUFFIX_CHARS` from `language::handle`;
the local suffix constant goes, and the base32 alphabet stays as the mint's own. The handle test
checks each minted handle with `HandleKind::minted`. `WorkspaceStore::lock` calls
`crate::sync::lock` (SERV-7); the two `Condvar` waits keep their inline recovery. `work/mod.rs`'s
three `stage` locks change with C10c, the next slice in that file, after LANG-3's hold.

WORK-7, the lease half: `WorkspaceLease::tab_owner` and `target_owner` had no caller and go; the
executor asks the store's versions. CompletionGate went with C19c. Left: the `Option` layer of
`substitute_references` in `work/flow.rs`, after LANG-3's hold.

C10c, one cross-lane commit (`language/catalog.rs` with the coordinator's leave): the frame's
`InvocationContext` keeps the `Publication` its snapshot came from, both set from one
`published()` in `InvocationContext::of`, and `policy_explain` reads that publication's compiled
answer instead of the facade's latest. `policy_explain_and_its_audit_line_name_one_authority`
publishes an edit between the frame's start and the explanation; read from the facade, the
explanation named the newer policy. `catalog_for` drops `browser_screenshot` while the snapshot
refuses screenshots (Decision 7.1); the catalog golden is unchanged, since screenshots are on by
default.

C12b-2, Decision 5.1's work half: `Terminal::refused` builds every refusal's result, with a
`Shape` (status, effect, readiness, repeat-safety) beside the typed `Refusal`, and writes
`refusal.reason()` as the `reason` fact. `blocked_at`, `failed`, `unknown`, `workspace_refusal`,
the unconfirmed action, `browser_failure`'s hand-built terminals, the routing refusals (which keep
their `for_gesture` audit), `page_refused`, the channel, capacity, start-deadline, cancellation,
credential and reverted-field refusals go through it, and the decode error keeps its decoder
guidance as its step. The 66 call-site literals, `browser_reason` and `unconfirmed_reason` are
gone. A temporary check, run over the whole suite before the literals went, found one written code
that differed from `reason()`; the unreached sites were read one by one. Model-visible on purpose,
pinned by tests, in no result golden: the credential refusal now carries
`credential_authorization_required`, an answer with no dialog open `no_dialog_visible`, and a
recording save the page did not take, or could not make, carries `recording_not_attached` or
`recording_export_failed`, with the browser's text in `detail`. An error that reaches
`browser_failure`'s last arm unhandled reports `browser_contract_failed`, the code of the contract
sentence it already showed. The unreachable selector let-else is an `expect`. Left for later rows:
the inline-ceiling recheck and the 7,000,000 cap (WORK-1), and the recovered and already-closed
tabs (Decision 4.5, WORK-3).

C16, first commit (Decision 6.7, cross-lane with the adapter's files under test-02's hold):
`WorkspaceState` keeps `recording: Option<HeldRecording { browser, id, saved }>`, and the store
reads, holds, marks and forgets it, since recording status, stop, save and discard run without the
lease. `work/recording.rs` runs the held-until-saved rule that `translate/records.rs` ran: the tab
still recording answers with its recording, a stopped one a save delivered is discarded and
replaced, one the browser lost is forgotten, and any other start is held. Every recording command
carries its id, `RecordingStarted` carries the new id, and the adapter keeps no index:
`RecordingIndex`, `Line.recordings` and `Link::recordings` are gone, and the wire is unchanged. A
session holding no recording is refused without asking a browser. Proofs:
`a_relay_restart_keeps_the_workspaces_recording_saveable`, `a_released_workspace_forgets_its_recording`,
the held start in `a_session_holds_one_recording_and_never_names_it` (nothing is started), and the
discard naming the saved recording in `a_session_starts_a_new_recording_after_saving_one`. The
adapter's three index tests moved to those executor tests, and
`each_recording_command_names_its_recording` pins the ids on the wire.

C16, second commit: `BrowserPort::call` and `call_guarded` lose their `workspace` parameter, which
existed only so the adapter could key the recording index (this amends ADR-0202 Decision 23, as
Decision 6.7 says). `Link::new` and `Link::workspace` follow, as do the executor's one caller,
`service/mod.rs`'s `cleanup_released_tabs` with its `"reaped"` and `"released"` placeholders and
`close_released`, `presentation/mod.rs`'s two calls, and every fake port: four in `work/tests`, three
in `workbench/tests.rs` (test-04's hold), the relay port's own and the contract tests'. Nothing on
the wire changes.
