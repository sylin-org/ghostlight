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

From 2026-10-21 (the owner: finish the pass before 2.0.1): test-02 takes BROW-1, BROW-5, BROW-8,
BROW-11 and BROW-13; the coordinator takes C3b, C3c, C19b and the SERV rows, C3b first.

S-D1, S-D2, SERV-7, S-D3 and S-D4 have landed. S-D2 fixed the update case, S-D3 the start on demand
after a Quit, and S-D4 the service an agent's tree kill took with it (Notes). Next: C19c-s, once C16
has landed in service/mod.rs.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| S-D1 | fix(intake): an opening that carries the largest resume token is accepted | - | SERV-1; the opening cap rises above the token cap, with a const assert, so tokens 2.0.0 edges hold pass too | landed (3db83135) |
| S-D2 | fix(service): a replacement waits for the browsers of the service it replaced | - | BROW-9; the test failed before the fix | landed (9c257daa) |
| SERV-7 | refactor: one poison-tolerant `lock` for the crate | - | SERV-7 and BROW-12's lock part; other lanes adopt it | landed (0ba38fa7) |
| S-D3 | fix(relay): a ready service wakes the edges that wait for it | - | BROW-9's open half, the owner's design (ADR-0199 amendment of 2026-10-20); relay revision 5 | landed (e4a9fade, b0290246) |
| S-D4 | fix(cli): a start relaunches itself, so a tree kill of its starter cannot reach the service | - | leo-desktop-02's finding while proving S-D3 (ADR-0199 amendment of 2026-10-21); cross-lane `cli/parse.rs`, `cli/mod.rs`, `cli/desktop.rs`, `cli/start.rs` under test-04's hold | landed (this commit) |
| S-D5 | fix(intake): a batch is answered as one array, its tool results in it | - | tests-1's service plan; the owner's question of 2026-10-21, sent back to the coordinator (Notes) | landed (this commit) |
| S-D6 | fix(intake): a client before MCP 2025-06-18 gets the lean JSON in a cut-off or expired answer too | - | tests-1's service plan (Notes) | landed (this commit) |
| S-D7 | fix(browser): a browser with no Ghostlight extension reads as the missing extension, with the step to add it | - | found on leo-desktop-02 and by test-01, 2026-10-21 (Notes); done by test-01, cross-lane into `work/` and `install/browser_profiles.rs` under test-03's and test-04's holds | landed (this commit) |
| S-D8 | fix(edge): a browser frame over the relay bound ends nothing | tests-1 T4 | test-02's answer to the coordinator's question, 2026-10-21 (Notes) | landed (this commit) |
| S-D9 | fix(edge): a client line over the relay bound is answered, in the service's words | S-D8 | the coordinator's relay and edge classification, 2026-10-21 (Notes) | blocked (after 2.0.1, the owner, 2026-10-21) |
| C19c-s | refactor(service): the service half of the completion path's leftovers | - | ADR-0203 C19c; SERV-5 | landed (this commit); no new test: diagnostics wording, ADR-0204 keeps the ceiling for promises |
| BROW-2 | refactor(browser): the dead recovery decisions go | C16 | BROW-2 | moved to test-01 (LEDGER-core-language.md), 2026-10-20 |
| C3b | fix(browser): a pinned wait reads its own browser | - | ADR-0203 C3b, moved from core | landed (this commit) |
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

S-D4: proving S-D3 on leo-desktop-02, a service the Copilot CLI's edge had started died about a
second after Copilot exited, with no `process_exited` line. The cause was neither a job object nor
the service: `taskkill /pid <edge> /T /F` took the service and its window, and stopping the edge
alone left both running. The service was the edge's child. Now `serve --reason R` refuses an
elevated token, starts its own image as `serve --reason R --relaunched` and exits 0, and the copy
serves, so the service has no living parent to be reached through. On Linux it is reparented to
init. The starters and the relay are unchanged. Tests: `cli::desktop` (the relaunch and the elevated
refusal before it), `cli::parse` (the relaunched form is the only extra one), and the race and
waiver journeys, which now find the service by its image (`processesOf` in `tests/lib/home.mjs`).
A kill-on-close job would need the relay to break away; none has been seen.
- S-D5 and S-D6, decided on 2026-10-21. The owner sent both questions back with "for Users (human
  and agentic) what's more delightful?". S-D5: an older client that batches expects JSON-RPC's
  answer, every response in the batch's one array, and a tool result sent outside it can be dropped
  by a strict client, whose agent then waits for an answer it was sent. The intake already holds a
  batch's array until its catalog requests are answered; it now holds it until its tool calls are
  too, and batches stay accepted at every revision (the leniency rule). S-D6: `intake/mcp.rs` builds
  the cut-off and expired templates at the latest revision, so a client before 2025-06-18 gets
  them without the lean JSON copy in the text block that C21 promises it for every other answer.
  Each lands with one test that reproduces it.
- S-D7, found on 2026-10-21: leo-desktop-02's Chrome started with no Ghostlight extension, and the
  agent was told "The Ghostlight adapter did not connect in time after the browser started." The
  outcome vocabulary already has `browser_extension_absent`, with its sentence and its step, but
  nothing constructs `RecoveryFailure::ExtensionAbsent` (test-01 found it unreachable while cutting
  `language/`). The browser profiles the installer reads say whether a profile holds the extension
  (`install/browser_profiles.rs`); recovery should use that, so a start into a browser with no
  Ghostlight extension says so and how to add it.
- S-D7 as landed: each installed browser the recovery inventory reads now says whether a profile
  of it holds the Ghostlight extension turned on, under the store id or the development id
  (`RecoveryCandidate::extension`). A browser Ghostlight started whose adapter never arrives is
  `ExtensionAbsent` when none does, and `HandshakeTimeout` as before when one does. The browser is
  still started either way, so a profile the scan misreads changes only the words, never whether
  Ghostlight starts the browser. `ExtensionAbsent` always gets the store step, setup finished or
  not. A development build loaded from a folder has no `Extensions` directory, so the profile scan
  also reads its settings' `location` (unpacked 4, command line 8).
- S-D7, follow-up after the live check on test-01 (2026-10-21, the owner's go): the extension was
  installed but turned off, and the agent was told the browser "does not have the Ghostlight
  extension installed" and to "add" it. The sentence now reads "does not have the Ghostlight
  extension installed and turned on", and its one step asks the user to "add or turn on Ghostlight
  in Browser" from the store listing or to open the Ghostlight window, whether or not setup
  finished. Only the handshake timeout still depends on setup for its advice.
- S-D8, found on 2026-10-21 by the relay and edge classification and confirmed by test-02: the
  browser relay ends its port on any Chrome frame over the relay bound (8 MiB), and that one port
  carries every session's browser. The extension bounds what it sends in UTF-16 characters, never
  the frame's UTF-8 bytes, and three answers can pass the bound on a large page: a read across
  documents (up to 255 frames of 50,000 characters in one receipt), an inspect across documents, and
  an accessibility snapshot (5,000 nodes, about 16 MB in CJK text). The edge reads the length header
  first, so it can skip the frame's bytes, log the size alone and keep the port, as `Reading::run`
  does for a frame it cannot decode (P-D5). The receipt is lost, so that call waits out its
  deadline, and every other session keeps its browser, with a 2.0.0 extension too. P-D6 (package)
  turns the wait into a refusal with a next step.
- S-D9, found the same day by the same classification: a client line over the relay bound is
  skipped up to its newline and never answered, so the agent waits forever. A line just under the
  bound whose frame is over it is answered from the `expired` template, whose words ("did not come
  back in time. Try the call again.") are wrong for it. An inline upload of about 6.3 MB or more
  reaches this, since the service's own 5 MB refusal never gets the line. The edge scans the
  skipped bytes for each request's id and method, whatever their order (the TypeScript SDK writes
  the id after the params), and answers each from a `too_large` template the service hands over
  with the others, in its own words with a next step (ADR-0197's Decision 2 gains the case). Before
  any reply, or from a service without that template, it answers as before any reply.
