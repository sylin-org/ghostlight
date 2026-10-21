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

S-D1, S-D2, SERV-7 and S-D3 have landed. S-D2 fixed the update case and S-D3 the start on demand
after a Quit (Notes). Next: C19c-s.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| S-D1 | fix(intake): an opening that carries the largest resume token is accepted | - | SERV-1; the opening cap rises above the token cap, with a const assert, so tokens 2.0.0 edges hold pass too | landed (3db83135) |
| S-D2 | fix(service): a replacement waits for the browsers of the service it replaced | - | BROW-9; the test failed before the fix | landed (9c257daa) |
| SERV-7 | refactor: one poison-tolerant `lock` for the crate | - | SERV-7 and BROW-12's lock part; other lanes adopt it | landed (0ba38fa7) |
| S-D3 | fix(relay): a ready service wakes the edges that wait for it | - | BROW-9's open half, the owner's design (ADR-0199 amendment of 2026-10-20); relay revision 5 | landed (e4a9fade, b0290246) |
| C19c-s | refactor(service): the service half of the completion path's leftovers | - | ADR-0203 C19c; SERV-5 | todo |
| BROW-2 | refactor(browser): the dead recovery decisions go | C16 | BROW-2 | moved to test-01 (LEDGER-core-language.md), 2026-10-20 |
| C3b | fix(browser): a pinned wait reads its own browser | - | ADR-0203 C3b, moved from core | todo |
| C3c | refactor(browser): one browser start, recorded in the launch guard | C3b, BROW-2 | ADR-0203 C3c, moved from core | todo |
| C18 | refactor(presentation): one table of activity and denial words, with the crossing names pinned | - | ADR-0203 C18, moved from core; SURF-4's label half; EXT-5's pins | moved to test-02 (LEDGER-package.md), 2026-10-20 |
| C19b | feat(audit): the record names the browser | I1 | ADR-0203 C19b, moved from core | todo |
| SERV-3 | refactor(service): `service/mod.rs` splits on its seams, and its tests move to `service/tests/` | C19c-s | SERV-3; the moves land alone | todo |
| SERV-8 | refactor(service): one end for the three exits | SERV-3 | SERV-8 | todo |
| SERV-10 | refactor(service): one start function | SERV-3 | SERV-10; the two browser handles stay for the tests' fake | todo |
| SERV-6 | refactor(intake): the service's vocabulary is read where the CLI reads it | - | SERV-6, both halves in one slice | moved to test-01 (LEDGER-core-language.md), 2026-10-20 |
| SERV-12 | refactor(diagnostics): one typed record | - | SERV-12; optional fields, relay's writer pinned by a round-trip test | moved to test-01 (LEDGER-core-language.md), 2026-10-21 |
| SERV-11 | refactor(service): one name per meaning on the service side | SERV-3 | SERV-11's service half | todo |
| SERV-14 | docs(service): the module docs map the crate | SERV-11 | SERV-14 | todo |
| BROW-1 | refactor(browser): protocol 4's frames and hello validators move into protocol 4 | C3c | BROW-1 as corrected; the registry stays in `browser/`; cross-lane `browser/adapter/` | todo |
| BROW-5 | refactor(browser): the `Work` alias goes | - | BROW-5 | todo |
| BROW-8 | refactor(browser): the port's literals are named | - | BROW-8's port half; `MAX_WORLD_CALLS >= DOCUMENT_LIMIT` as a check, not a tie | todo |
| BROW-11 | docs(browser): the port's docs match the code | BROW-1 | BROW-11's port half | todo |
| BROW-12 | refactor(browser): one SHA-256 helper | - | BROW-12's hash part | landed with the adapter's BROW-12a (52753883); nothing of it is left in this lane's files, and governance's own hex writers are policy's |
| BROW-13 | refactor(browser): the display-session module says what it is | - | BROW-13 as corrected: the `session.rs` rename and the golden path | todo |

## Notes

SERV-2 and SERV-4 are in `cli/`, which is install's. SERV-9 goes with core-work's C16. SERV-13 is
install's.

S-D2 (BROW-9): the defect is real. A new service has no record of the old one's departures, so
after a replacement only the window from its publication counts, and each relay of the ended
service looks for it at 0.5, 1.5, 3.5 and 7.5 s after the loss. A replacement that publishes
later than 1.5 s left a call with no bound browser to recovery before the relay's next look: on
Windows Ghostlight ran Chrome, which opened a second window in the running instance; on Linux the
agent read `browser_startup_manual` for an open browser. The fix: a start with reason `update` or
`recover` (`lifetime::replaces_a_service`) publishes through
`RelayBrowserPort::replacement_published`, which expects the relays for `RETURN_GRACE` when the
home remembers a browser, and for `RELAY_RETURN_WINDOW` otherwise, so a first install keeps its
startup budget. Tests: `service::tests::a_replacement_waits_for_the_relays_of_the_service_it_replaced`
(failed before the fix), `a_start_that_replaced_no_browser_waits_only_the_relay_return_window` and
`lifetime::tests::only_an_update_or_recover_start_replaces_a_service`. ARCHITECTURE, ACCEPTANCE
and LANGUAGE say so.

SERV-7: `crate::sync::lock` replaces this lane's eight copies and its direct locks in
`browser/transport.rs`, `service/test_home.rs`, `audit.rs` and `presentation/mod.rs`. Condvar
waits keep their inline `PoisonError::into_inner`, because a wait takes a guard and returns one.
The other lanes adopt `lock` in their own files: adapter in `protocol4/fake.rs` (a copy), policy
in `governance/mod.rs` (a copy), install in `install/mod.rs` (a copy), `install/icons.rs` and
`agent/image.rs`, core-work in `work/mod.rs` and `workspace/mod.rs`, surface in `workbench/**`.
`crates/relay` and `crates/edge` keep theirs (Decision 13).

Open, the owner's choice: a demand start long after a Quit. The relay of an open browser keeps
looking every 30 s, and a call that starts the service lands within about a second of its
publication, so in about 97% of cases the relay's next look falls outside any window a call's
7 s grace can hold. On Windows a second window opens and most such calls end in
`browser_handshake_timeout`; on Linux they read `browser_startup_manual`. The relay is locked
(Decision 13). The ways out: the extension reopens its port on its own schedule while the service
is absent (package lane), or recovery declines to start a browser whose relay is still alive and
says it is reconnecting (a new refusal word, and a way to know the relay is alive).

S-D3 settles it the owner's way, on 2026-10-20: event-driven, with a check fallback, and the timing
left to the coordinator. The browser and the extension still know nothing of the service. A
waiting edge registers a wake endpoint in `run/wake` (`ghostlight_relay::wake`), the service wakes
every registered edge once its endpoint accepts openings (`publish_ready`), and a woken edge looks
at once. The backoff stays, and both sides log the wake (`service_ready`, `edges_woken`). The edge
changes, so relay revision 5 replaces 4 and the service admits both (`intake/listener.rs`
`OLDEST_RELAY`). ADR-0203 Decision 13 is amended for this one change. Tests: the wake module's in
`crates/relay`, the edges' wake tests in `port/tests.rs` and `forward/tests.rs`, `golden5.rs`, the
listener's revisions test and the service's publication test. The live proof runs on a test
machine: Quit, wait past 30 s, an agent call, and the browser is back within a second with no
second window. Done on 2026-10-21. On leo-desktop-02 Chrome attached 57 ms after the service
was ready, with no second window, and `edges_woken woken=2` logged beside `service_ready`. On
Bluefin, run 2 attached the browser 27 ms after the start, with no `browser_startup_manual`. The
Linux relay and edge tests pass. A woken browser edge exits without removing its registration,
and the next start sweeps it (`removed=1`), as the wake module says.
