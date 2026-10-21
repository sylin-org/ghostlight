# arch-1: core-service ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Files: `service/**`,
`intake/` apart from `intake/desktop.rs`, `browser/` outside `browser/adapter/`,
`presentation/**`, `audit.rs`, `audit/**`, `events.rs`, `provenance.rs`, `diagnostics.rs`,
`lib.rs`, `main.rs`, the journeys in `tests/`, `tests/lib/**` and `tests/guards/**`. Machine: the
coordinator, on the owner's machine; journeys that start a service run on a test machine. A
slice's specification is its finding in [sweep-2026-10-20/service.md](sweep-2026-10-20/service.md)
or [sweep-2026-10-20/browser.md](sweep-2026-10-20/browser.md), read with its skeptic's
corrections, or its entry in [ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan.
Statuses: `todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

S-D1 has landed. Next: S-D2.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| S-D1 | fix(intake): an opening that carries the largest resume token is accepted | - | SERV-1; the opening cap rises above the token cap, with a const assert, so tokens 2.0.0 edges hold pass too | landed (this commit) |
| S-D2 | test(browser): the relay's return window covers its backoff after an update | - | BROW-9; a fix follows if the test fails | todo |
| SERV-7 | refactor: one poison-tolerant `lock` for the crate | - | SERV-7 and BROW-12's lock part; other lanes adopt it | todo |
| C19c-s | refactor(service): the service half of the completion path's leftovers | - | ADR-0203 C19c; SERV-5 | todo |
| BROW-2 | refactor(browser): the dead recovery decisions go | - | BROW-2 | todo |
| C3b | fix(browser): a pinned wait reads its own browser | - | ADR-0203 C3b, moved from core | todo |
| C3c | refactor(browser): one browser start, recorded in the launch guard | C3b, BROW-2 | ADR-0203 C3c, moved from core | todo |
| C18 | refactor(presentation): one table of activity and denial words, with the crossing names pinned | - | ADR-0203 C18, moved from core; SURF-4's label half; EXT-5's pins | todo |
| C19b | feat(audit): the record names the browser | I1 | ADR-0203 C19b, moved from core | todo |
| SERV-3 | refactor(service): `service/mod.rs` splits on its seams, and its tests move to `service/tests/` | C19c-s | SERV-3; the moves land alone | todo |
| SERV-8 | refactor(service): one end for the three exits | SERV-3 | SERV-8 | todo |
| SERV-10 | refactor(service): one start function | SERV-3 | SERV-10; the two browser handles stay for the tests' fake | todo |
| SERV-6 | refactor(intake): the service's vocabulary is read where the CLI reads it | - | SERV-6, the intake half; the CLI half is install's | todo |
| SERV-12 | refactor(diagnostics): one typed record | - | SERV-12; optional fields, relay's writer pinned by a round-trip test | todo |
| SERV-11 | refactor(service): one name per meaning on the service side | SERV-3 | SERV-11's service half | todo |
| SERV-14 | docs(service): the module docs map the crate | SERV-11 | SERV-14 | todo |
| BROW-1 | refactor(browser): protocol 4's frames and hello validators move into protocol 4 | C3c | BROW-1 as corrected; the registry stays in `browser/`; cross-lane `browser/adapter/` | todo |
| BROW-5 | refactor(browser): the `Work` alias goes | - | BROW-5 | todo |
| BROW-8 | refactor(browser): the port's literals are named | - | BROW-8's port half; `MAX_WORLD_CALLS >= DOCUMENT_LIMIT` as a check, not a tie | todo |
| BROW-11 | docs(browser): the port's docs match the code | BROW-1 | BROW-11's port half | todo |
| BROW-12 | refactor(browser): one SHA-256 helper | - | BROW-12's hash part | todo |
| BROW-13 | refactor(browser): the display-session module says what it is | - | BROW-13 as corrected: the `session.rs` rename and the golden path | todo |

## Notes

SERV-2 and SERV-4 are in `cli/`, which is install's. SERV-9 goes with core-work's C16. SERV-13 is
install's.
