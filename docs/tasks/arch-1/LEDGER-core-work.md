# arch-1: core-work ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Files: `work/**` and
`workspace/**`. Machine: test-03 (Alpine). A slice's specification is its finding in
[sweep-2026-10-20/work.md](sweep-2026-10-20/work.md), read with its skeptic's corrections, or its
entry in [ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`,
`landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Nothing has landed. Start with W-D1.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| W-D1 | fix(work): an inline upload of exactly 5,000,000 bytes is accepted | - | WORK-1's note | todo |
| W-D2 | fix(work): a flow step whose tab close the policy refuses shows the kept-tab notice | - | WORK-8's notice half | todo |
| W-D3 | fix(work): a selector wait gives the readiness fact the other waits give | - | WORK-13; a deliberate model-visible change | todo |
| C19c | refactor(work): the completion path's leftovers go | - | ADR-0203 C19c, moved from core; WORK-7's `CompletionGate`; the service half is core-service's | todo |
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
| WORK-12 | refactor(workspace): its tests and its two books move out, and the lease helpers come in | - | WORK-12; the moves land alone | todo |
| WORK-5 | refactor(work): the closed vocabularies are matched as types | LANG-3 | WORK-5 | todo |
| WORK-2b | refactor(work): `HandleKind` at the mint sites, and the crate `lock` helper here | LANG-2, SERV-7 | LANG-2 and SERV-7's adoption | todo |

## Notes

WORK-14 is not taken: browser events change leased workspaces through the store by design, and C4
recorded the choice. WORK-8's typed coverage is not taken either; ADR-0203's audit-projection
route stands. WORK-11 and WORK-15 are policy's (LEDGER-policy.md).
