# arch-1: core-work ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Files: `work/**` and
`workspace/**`. Machine: test-03 (Alpine). A slice's specification is its finding in
[sweep-2026-10-20/work.md](sweep-2026-10-20/work.md), read with its skeptic's corrections, or its
entry in [ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`,
`landed` (with commits), `blocked` (with the reason).

## RESUME HERE

W-D2, W-D3 and C19c have landed, and WORK-12's three moves (its helpers are still to come).
W-D1 moved to core-language (L-D5). Next: C10c (P1 and P5 have landed). A slice's hash is
filled in by the next slice's commit, since a rebase before the push changes it.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| W-D1 | fix(work): an inline upload of exactly 5,000,000 bytes is accepted | - | WORK-1's note | moved to LEDGER-core-language.md (L-D5) |
| W-D2 | fix(work): a flow step whose tab close the policy refuses shows the kept-tab notice | - | WORK-8's notice half | landed (ecf30a64) |
| W-D3 | fix(work): a selector wait gives the readiness fact the other waits give | - | WORK-13; a deliberate model-visible change | landed (f6d9fdc3) |
| C19c | refactor(work): the completion path's leftovers go | - | ADR-0203 C19c, moved from core; WORK-7's `CompletionGate`; the service half is core-service's | landed |
| C10c | refactor(work): the executor reads one publication | P1, P5 | ADR-0203 C10c, moved from core | todo |
| C16 | refactor(work): which recording a workspace holds is a workspace fact | - | ADR-0203 C16, moved from core; removes SERV-9's "reaped" | todo |
| C12b-2 | refactor(work): the typed refusal builds its result (the work half) | C12b-1 | ADR-0203 C12b; WORK-4, WORK-2 and WORK-3's refusal sites | todo |
| WORK-3 | refactor(work): the typed outcome builds its result too | C12b-2 | WORK-3; four sites keep their overrides | todo |
| WORK-1 | refactor(work): the upload ceiling has one home | W-D1 | WORK-1; the screenshot cap reads the adapter's `MAX_BASE64_CHARS` | todo |
| WORK-10 | refactor(work): timings and limits are named | - | WORK-10 | todo |
| WORK-7 | refactor(work): dead code goes | C19c | WORK-7; the two unused lease methods go | todo |
| WORK-9 | refactor(work): one name per contract | - | WORK-9; no persisted word changes | todo |
| WORK-13 | refactor(work): one finish for every wait | W-D3 | WORK-13 | todo |
| WORK-6 | refactor(work): `work/mod.rs` splits on its seams | WORK-3 | WORK-6; the moves land alone | todo |
| WORK-12 | refactor(workspace): its tests and its two books move out, and the lease helpers come in | - | WORK-12; the moves land alone | moves landed (8108ea82, 66a8842b, c65ec18d); helpers todo |
| WORK-5 | refactor(work): the closed vocabularies are matched as types | LANG-3 | WORK-5 | todo |
| WORK-2b | refactor(work): `HandleKind` at the mint sites, and the crate `lock` helper here | LANG-2, SERV-7 | LANG-2 and SERV-7's adoption | todo |

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
