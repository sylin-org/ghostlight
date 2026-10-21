# Second sweep, 2026-10-20: service, intake and cli

This sweep read `crates/orchestrator/src/service/` (about 10,500 lines, of which `service/mod.rs`
alone is 7,400, the largest file in the repository), `crates/orchestrator/src/intake/` (about
3,000 lines: the MCP intake, the opening reader and the relay listener) and
`crates/orchestrator/src/cli/` (about 7,200 lines: the command line, setup and doctor), plus
`main.rs` and `lib.rs` for how the crate is organized at the top. It was read-only, on `dev` at
`8f6a43ec`. Nothing was edited, built, run or committed. Where a finding crosses into
`workspace/`, the browser adapter, `crates/bridge`, `crates/relay` or `crates/edge`, that code was
read too. Tests pin the command-line words and the doctor output, so every section says on its
own line whether it changes user-visible text. Where a finding meets a decision of ADR-0203, the
section says so. Paths are relative to `crates/orchestrator/src/` unless they begin with
`crates/`, `docs/` or `tests/`. Line numbers are as of `8f6a43ec`. Findings are ranked by payoff
over risk.

## SERV-1: A resume token can make its own opening too large

Kind: magic limits with an unstated relation; a latent bug.

Where:

- `MAX_RESUME_TOKEN_BYTES` (`intake/mcp.rs:69`) and `MAX_OPENING_BYTES` (`intake/opening.rs:17`),
  both 64 KiB.
- `ResumeState::encode` (`intake/mcp.rs:713`), which trims a large token.
- `read_opening` (`intake/opening.rs:44`, the size check at 49 to 51) and `listener::judge`
  (`intake/listener.rs:107`), which refuses a `TooLarge` opening as malformed.
- The edge's connection attempt (`crates/edge/src/forward.rs:357` to 384): it sends the kept
  token at 364 and only ever sets it, at 531.
- The size test (`intake/mcp/tests.rs:795`).

Problem:

- Both limits are 64 KiB, but the resume token travels inside the opening (`Opening.resume`).
- When a session holds many tabs, `encode` trims the token to the most tabs that fit under
  `MAX_RESUME_TOKEN_BYTES`. A trimmed token therefore lands within one tab's encoded size of the
  limit.
- The opening adds about 100 bytes of envelope (estimated from
  `tests/golden/relay4/opening.mcp.json`), so the opening can exceed `MAX_OPENING_BYTES`.
  `read_opening` then returns `TooLarge`, and `judge` refuses the opening as malformed.
- The edge, whose code is frozen, never clears its token. It retries the same opening with backoff
  forever, so that session can never reconnect.
- The size test checks only the token's length, not that an opening carrying it is read. No
  per-session tab cap was found that keeps the token small.
- The relay protocol's bounds (`docs/design/relay-protocol-4.md`, section 10) name neither limit,
  so the fix is service-only.

Proposed change: encode to `MAX_OPENING_BYTES - ENVELOPE_BYTES`, with a `const` assert that ties
the two limits, and add a test that reads an opening carrying a trimmed token. Decode keeps the
opening's limit, so a token an older service wrote still decodes.

Ratings: payoff high, risk low, effort S. User-visible text: none.

Verdict: confirmed. Encode bounds only the token (`intake/mcp.rs:730-745`), and the edge's compact
opening adds 107 bytes for build `2.0.1` (`crates/edge/src/forward.rs:634-642`; the golden's
`reason` is not sent), so a token in the top 107 bytes under 64 KiB makes a frame `read_opening`
refuses from its header, and `judge` answers `malformed`. It takes one session holding about 269
to 281 bound tabs (230 to 243 token bytes each, by claim length and tab-id digits), and then only
in about half the configurations; an untrimmed token in that band fails too. Only a token that
keeps tabs matters, so after a `replace`, a crash or a lost stream; a quit's token has none. The
edge then retries only while a request is held or a replace is expected (`forward.rs:274-276`),
backs off 0.5 s to 30 s (`crates/relay/src/lib.rs:45,48`), answers each held request as expired
after 30 s (`lib.rs:42`), and outside the replace window each attempt starts a
`serve --reason agent` that waits 5 s for the lock and gives way (`forward.rs:373-381`). The
session stays stuck until its client restarts the server. Fix correction: v2.0.0 already ships
this encode (C11's 8c216920 is in the tag), so shrinking only the encoder leaves a 2.0.0 edge's
held token refused after the update, before decode runs. Raising `MAX_OPENING_BYTES` to
`MAX_RESUME_TOKEN_BYTES` plus an envelope bound that covers longer builds, tied by a `const`
assert, fixes both. The cap is in neither the relay design's section 10 nor
`tests/golden/relay4/vocabulary.json`, and 16 handshakes (`service/admission.rs:18`) bound its
memory.

Class: defect

arch-1: C11 (landed, 8c216920) was meant to cover it: it bounds the token on write, and its proof
`a_token_never_outgrows_the_opening_cap` (`intake/mcp/tests.rs:768`) checks only the token cap.
This is a fix-up of C11, not new scope.

## SERV-2: Timing constants are tied together only in comments, and one tie is broken

Kind: magic numbers with unstated relations across modules.

Where:

- `UNINSTALL_QUIT_WAIT` (`cli/setup.rs:119`) and its wait loop in `stop_running_for_uninstall`
  (`cli/setup.rs:133` to 152).
- `QUIT_GRACE` and `END_LIMIT` (`service/lifetime.rs:20,25`), and `serve_control`, which begins
  every control quit with `QUIT_GRACE` (`service/mod.rs:1389`).
- `RunningAfterUninstall::StillRunning` and its sentence (`language/setup.rs:1116`).
- `SHOW_PATIENCE` (`service/presence.rs:57` to 59) and `WORKBENCH_ACTIVATION_TIMEOUT`
  (`cli/desktop.rs:34` to 37).
- `UPDATE_LOCK_WAIT`, `LEAVING_LOCK_WAIT` and `READY_LIMIT` (`service/lifetime.rs:22` to 42), and
  `SERVICE_READY_TIMEOUT` (`cli/desktop.rs:32`).

Problem:

- `UNINSTALL_QUIT_WAIT` is 10 s. A quit can drain for `QUIT_GRACE` (10 s) and then take up to
  `END_LIMIT` (5 s) to hand out tokens, say goodbye and release tabs.
- A whole uninstall with a call in flight therefore prints "The running Ghostlight did not stop in
  time; quit it from its icon, or sign out." for a service that is still leaving.
- Three other ties live only in prose:
  - `SHOW_PATIENCE` (10 s) must stay below `WORKBENCH_ACTIVATION_TIMEOUT` (15 s).
  - `QUIT_GRACE + END_LIMIT` must fit inside `UPDATE_LOCK_WAIT` (30 s).
  - `SERVICE_READY_TIMEOUT` (30 s) is shorter than the worst legal start, `LEAVING_LOCK_WAIT`
    (5 s) plus `READY_LIMIT` (30 s), and no comment says whether that is intended.

Proposed change: derive the command line's waits from the constants in `service/lifetime.rs`
(for example `UNINSTALL_QUIT_WAIT` from `QUIT_GRACE + END_LIMIT` plus a margin), and put
`const _: () = assert!(..)` checks beside those constants for the other ties.

Ratings: payoff med-high, risk low, effort S. User-visible text: no words change, but the false
"did not stop in time" line stops appearing.

Verdict: confirmed. The 10 s wait starts once the quit is answered (`cli/setup.rs:138-144`), which
`serve_control` does right after `begin` spawns the drain (`service/mod.rs:1389-1398`). The quit
waits up to `QUIT_GRACE` for work in flight (`service/lifetime.rs:169`), ends under `END_LIMIT`
(`service/mod.rs:931-956`), and frees the lock only after the host's join (`cli/desktop.rs:167`,
`service/mod.rs:629-648`): about 15 s at worst. The trigger is narrower than "a call in flight":
drain and end together must pass 10 s, as when a fill or a recording (timeout up to 30 s,
`language/mod.rs:49-51`) still runs when the grace ends. The line's advice then names a tray icon
that the end's shell goodbye removes (`service/mod.rs:953`). The other three ties are prose only,
as stated (`service/presence.rs:57-58`, `service/lifetime.rs:22-24`; `cli/desktop.rs:32` has no
comment). The change is safe, since the loop returns as soon as the lock is free. Deriving
`SERVICE_READY_TIMEOUT` would stretch a stalled start's wait from 30 s to 35 s, the owner's call.

Class: defect, refactor

arch-1: none. C5 (landed, 9beb7976) added `END_LIMIT` and tied it only to the update start's 30 s
(LEDGER-core.md:196-200); `UNINSTALL_QUIT_WAIT` came earlier, in e964e858.

## SERV-3: service/mod.rs is about eight modules plus a test suite

Kind: structure.

Where: `service/mod.rs`, 7,400 lines, and `service/toolset.rs`, which imports
`super::{lock, McpWire}`.

Problem:

- Lines 2442 to 7400, 4,959 lines or two thirds of the file, are one test module.
- The 2,441 production lines hold eight concerns that already have clear seams, and
  `toolset.rs` reaches back into the parent for the stream type.

Proposed change:

- Move the tests to `service/tests/` by topic, as `intake/mcp/tests.rs` and the
  `connection/tests.rs`, `link/tests.rs` and `translate/tests.rs` files under
  `browser/adapter/chromium/protocol4/` already do.
- Split the production code on its seams:
  - `quit.rs`: lines 657 to 1043 (`Quit`, `Farewell`, `FarewellState`, `Enrolled`, `quit_cause`,
    `goodbye_for`).
  - `mcp_stream.rs`: lines 1402 to 2129 less the refresher at 1798 to 1821, plus `deliver_result`
    at 2300 (`serve_mcp`, `Accepted`, `McpWire`, `Resume`, `LiveEnd`, `KeptTabs`, `kept_tabs`,
    `AdvertisedCatalog`, `attached`, `LiveSession`, `remember_resumed_tabs`). In its own module,
    `serve_mcp`'s run-once closure (`served = (|| ...)()`, 1497 to 1538) and the `opened` flag
    that `open_when_initialized` captures (1502 to 1511) can become methods of a stream type.
  - Desktop serving into `shell.rs`: lines 1178 to 1349 (`serve_desktop`, `ShellPresent`,
    `explain_close`, `TrayRelay`).
  - `connections.rs`: `Intake` (1045 to 1091), `spawn_listener` and `serve_connection` (1093 to
    1176), `control_status` and `serve_control` (1351 to 1400).
  - `release.rs`: lines 2201 to 2296 (`reap_finished_sessions`, `cleanup_released_tabs`,
    `owner_alive`).
  - `browser_events.rs`: lines 2322 to 2440 (`ServiceBrowserEvents`, `reveal_once`,
    `reveal_off_thread`).
  - `mod.rs` keeps `ServiceHost`, `StartTerms`, the refresher, the start gate and the start
    housekeeping (2131 to 2199).
- `toolset.rs` then imports its siblings.
- Land these as pure moves in their own commits, kept apart from the arch-1 core lane's open
  slices that edit this file, C19c among them (SERV-5).

Ratings: payoff high, risk low, effort M. User-visible text: none.

Verdict: confirmed. The test module is lines 2442 to 7400 (4,959 lines), each cited range starts
and ends at the items it names, and `toolset.rs:34` imports `super::{lock, McpWire}`. Two notes:
open core slice C16 edits this file too (the "reaped" and "released" placeholders at
`service/mod.rs:2204,2672,2706,2841`), so the moves wait for or stay apart from C16 as well as
C19c; and the precedent for tests by topic is `work/tests/`, since the protocol4 ones are single
`tests.rs` files. Pure moves need `pub(super)` on the items the new siblings share.

Class: refactor

arch-1: none

## SERV-4: The call command is parsed outside the parser

Kind: structure and naming, with one latent panic.

Where:

- `LaunchMode::Call` (`cli/parse.rs:282`) and `run_call` (`cli/desktop.rs:295` to 312).
- `main` (`main.rs:16` to 17), `listen_unless_serving` (`cli/session_end.rs:22`) and `start`
  (`cli/start.rs:73`).
- `cli::parse` the module (`cli/mod.rs:13`) and `cli::parse` the function (`cli/mod.rs:81`).
- The exit codes (`cli/mod.rs:35` to 41) and `READY_STALLED_EXIT` (`cli/desktop.rs:39`).
- `dispatch` (`cli/mod.rs:283`, `Call` at 291 and `Policy` at 293) and `run_policy`
  (`cli/desktop.rs:325`).
- `launch_mode` (`cli/parse.rs:168`) and `SUBCOMMANDS` (`cli/parse.rs:21`).

Problem:

- `LaunchMode::Call` carries nothing, so `run_call` re-reads `std::env::args()`, skips to "call"
  again with `skip_while`, and exits with a literal `1` (`cli/desktop.rs:305`) instead of
  `EXIT_USAGE`.
- `std::env::args()` panics on a non-UTF-8 argument, where `main` uses `args_os`.
- `main` parses argv twice before dispatch: once in `listen_unless_serving` and once in `start`.
- `cli::parse` is both a module and a function. `cli::Command` is call's command, beside
  `cli::diagnostics::Command`, `governance::inspection::Command` and the `std::process::Command`
  that `cli/start.rs` imports.
- The `cli/mod.rs` comment "The codes are stable for scripts; 3 is not used." is wrong:
  `READY_STALLED_EXIT` is 3.
- `dispatch` sends `Call` and `Policy` through `cli/desktop.rs`, where `run_policy` only forwards
  to `governance::inspection::run`.
- `launch_mode` is 156 lines of if/else:
  - the subcommand words are spelled once in `SUBCOMMANDS` and again in the arms;
  - `--help`/`-h` is spelled three times (`cli/parse.rs:200,210,278`);
  - the native-host usage is written twice (222, 225), and the UTF-8 sentence twice (250, 332);
  - option values are sliced with `&value["--x=".len()..]` three times (`cli/parse.rs:347,361`,
    `cli/mod.rs:99`).

Proposed change:

- A `cli/call.rs` owns call's command, parser, runner and exit codes, and `parse.rs` produces
  `LaunchMode::Call(call::Command)`.
- `cli/mod.rs` keeps only the module list and `dispatch`. `Policy` dispatches straight to
  `governance::inspection`. One table lists every exit code, `serve`'s 3 included.
- `main` parses once and hands the result to `listen_unless_serving` and `start`.
- `launch_mode` becomes one `match` over word constants that `SUBCOMMANDS` shares, with a
  help-flag helper, one constant per repeated sentence, and `strip_prefix` for `--x=` values.

Ratings: payoff high, risk low, effort M. User-visible text: none, except that a non-UTF-8
argument gets a usage error instead of a panic.

Verdict: partial. Holds: `run_call` re-reads `std::env::args()` (`cli/desktop.rs:296-300`) and
exits with a literal 1 equal to `EXIT_USAGE` (`cli/mod.rs:37`); the call arm of `launch_mode`
checks no UTF-8 (`cli/parse.rs:275-283`), so a non-UTF-8 argument reaches `run_call` and panics
(exit 101); argv is parsed twice before dispatch; the repeats are where cited. Corrections: the
comment "3 is not used" (`cli/mod.rs:35`) is about call's status codes, where 3 is unused, as the
man page lists them (`packaging/linux/man/ghostlight.1:124-145`); `READY_STALLED_EXIT`
(`cli/desktop.rs:39`) is the exit of `serve`, which only Ghostlight's programs run, so it does not
belong in call's table. And moving call's parse into `launch_mode` changes what a person sees
unless the change says otherwise: a parse error would leave through `main`'s `anyhow::Result`
(`main.rs:15`) as "Error: ...", where it now prints the bare line (`cli/desktop.rs:304`), and a
call this copy cannot parse would get `Role::Inspect` (`cli/start.rs:73-74`) and run here, not be
forwarded to the installed service or carrier-installed first (`install/entry.rs:100-107`).

Class: defect, refactor

arch-1: none

## SERV-5: Decided but not done: ADR-0203 slice C19c

Kind: dead names and stale docs, already decided.

Where:

- The `eprintln!` to the service's null stderr in `serve_connection` (`service/mod.rs:1159`).
- `desktop_ready=true` in `published_detail` (`service/mod.rs:121`), and the test named after it
  (`service/mod.rs:4741`).
- The stale `AuthorityAlreadyRunning` doc (`service/mod.rs:205`).
- `HARNESS_DETACHED` naming the workspace instead of the connection (`service/mod.rs:2104`).
- The session watch's `generation` counter (`service/mod.rs:1968,1983`) and `CatalogChanged`'s
  fields (`intake/session.rs:195` to 200).

Problem: ADR-0203 Decision 13 and its Rebuild-plan entry C19c remove each of these.
`docs/tasks/arch-1/LEDGER-core.md` lists C19c as `todo`, gate none, after ux-2 closes.

Proposed change: land C19c from that ledger as specified, not as new work from this sweep.

Ratings: payoff med, risk low, effort S. User-visible text: diagnostics lines change. The
published line loses `desktop_ready=true`, the detached line names the connection, and a session
that ends in error writes a diagnostics line.

Verdict: confirmed. Every item is at the cited line (`service/mod.rs:121,205,1159,1968,1983,2104`
and the test at 4741), `CatalogChanged`'s fields are matched only as `{ .. }`
(`intake/mcp.rs:378`), and C19c's Changes and Deletes name all five. The ledger row is `todo`, gate
none. No test or journey reads `desktop_ready` or the detached line's detail.

Class: refactor, persisted

arch-1: C19c (todo, gate none)

## SERV-6: The command line spells the service's vocabulary again on the reading side

Kind: repeated literals for a vocabulary that crosses a process boundary.

Where:

- `JSONRPC_VERSION` and `IMAGE_CONTENT` (`cli/client.rs:32,35`), against `JSONRPC_VERSION`
  (`intake/mcp.rs:65`) and `content::IMAGE` (`intake/mcp.rs:119`).
- The `structuredContent` and `content` literals the client reads (`cli/client.rs:237,252`),
  against the result the engine writes (`intake/mcp.rs:874`).
- `exit_code` (`cli/mod.rs:267`), against `work::result::Status` (`work/result.rs:11` to 13).
- `SUPPORTED_REVISIONS` (`intake/mcp.rs:53`), against `STRUCTURED_CONTENT_REVISION`
  (`intake/mcp.rs:57`).

Problem:

- The command line is an MCP client in the same crate as the engine, yet it redefines the
  engine's constants and reads the engine's field names as literals.
- `exit_code` matches the strings "succeeded", "blocked", "failed" and "cancelled", although
  `Status` derives `Deserialize` with the same snake_case names. A new status would fall to
  `EXIT_UNKNOWN` without a compiler error.
- `SUPPORTED_REVISIONS` repeats "2025-06-18" beside the constant that names it.

Proposed change: export the engine's constants as `pub(crate)` and use them in the client; decode
`Status` and match it exhaustively; write `STRUCTURED_CONTENT_REVISION` inside
`SUPPORTED_REVISIONS`.

Ratings: payoff med, risk low, effort S. User-visible text: none.

Verdict: confirmed. The client repeats `intake/mcp.rs:65,119` at `cli/client.rs:32,35`, reads
`content` and `structuredContent` as literals (`cli/client.rs:237,252`), and `exit_code` matches
status words (`cli/mod.rs:267-275`). The lean view always keeps `status`
(`intake/mcp/lean.rs:5-6`), so the mapping works today. The decode must still map a word this
build does not know, as from a service of another build, to `EXIT_UNKNOWN`.

Class: refactor

arch-1: none

## SERV-7: Copy-paste repetition in the service

Kind: duplicated code.

Where:

- The poison-tolerant `fn lock` helper at `service/mod.rs:2316`, `service/idle.rs:248`,
  `service/lifetime.rs:248`, `service/presence.rs:141` and `service/shell.rs:40`: 17 copies in
  the crate, two of them in test support.
- The 14 `emit` calls in `service/mod.rs`, against the import at `install/icons.rs:37`.
- Admit-or-refuse in `serve_connection` (`service/mod.rs:1139` to 1165).
- `serde_json::to_value(..).unwrap_or(Value::Null)` in `serve_control` (`service/mod.rs:1374` to
  1391).

Problem:

- The same five-line `lock` helper is copied 17 times.
- Every `emit` call in `service/mod.rs` spells `ghostlight_bridge::diagnostics::event::` and
  `ghostlight_bridge::diagnostics::Level::` in full, five or six lines each, where
  `install/icons.rs` imports `{event, Level}`.
- Several of those calls reuse `PROCESS_FAILED` for failures that are not the process failing:
  the tray-hint marker at `Warn` (`service/mod.rs:1298` to 1303), the quit latch write (701 to
  706) and the drain thread (734 to 739). `HARNESS_ATTACHED` carries the unreadable-token warning
  (1439 to 1446). Choosing better events is a diagnostics vocabulary change and is left out here.
- `serve_connection` repeats its admit-or-refuse block 4 times, and `serve_control` repeats its
  serialization fallback 4 times.

Proposed change: one crate-level `lock` helper; import `{event, Level}`; admit once for the four
roles that are served; serialize once after the match.

Ratings: payoff med, risk low, effort S. User-visible text: none.

Verdict: confirmed. There are 17 generic `fn lock<T>` copies, two under `#[cfg(test)]`
(`browser/adapter/chromium/protocol4/fake.rs:279`, `browser/mod.rs:1781`); the 14 production
`emit` calls spell both bridge paths in full; the admit block (1139-1165) and the fallback
(1374-1391) repeat 4 times each. The copies sit in five lanes (core, adapter, policy, install,
surface), so one crate-level helper is a cross-lane change under BOOTSTRAP's rule. The
`HARNESS_ATTACHED` warning was C11's choice, to add no bridge event (LEDGER-core.md:448-450).

Class: refactor

arch-1: none

## SERV-8: Three exit paths share one tail, written three times

Kind: duplicated lifecycle logic.

Where:

- `Quit::begin` (`service/mod.rs:694`), `Quit::session_end` (747) and `Quit::idle_exit` (776).
- The quit signal fired at 728 to 731, 756 to 759 and 788 to 790.
- The thread-start failures at 733 to 740 and 761 to 768.
- The `begin_quit` calls at 695, 748 and 777.

Problem:

- Each path calls `drain.begin_quit`, closes the shells, runs a `Farewell` variant, then clones the
  quit signal under its lock and fires it.
- `begin` and `session_end` each spawn a named thread and emit `PROCESS_FAILED` when it cannot
  start.
- Only `session_end` ignores `begin_quit`'s answer, while `begin` and `idle_exit` return when a
  quit is already under way. The code does not say whether that difference is deliberate.

Proposed change: one private `end_with(cause, how)` and one `fire_signal()`, with the rule for a
session end that arrives during a quit stated in code.

Ratings: payoff med, risk low-med, effort S. User-visible text: none.

Verdict: confirmed. The three signal tails, the two thread-start failures and the three
`begin_quit` calls are at the cited lines, and only `session_end` drops the answer
(`service/mod.rs:748`). No test covers a sign-out during a quit; the nearest is
`the_end_of_the_session_leaves_once_and_the_idle_path_adds_nothing` (`service/mod.rs:7360`). A
merged helper must keep `session_end` off the drain, since `FALLBACK_EXIT` (2 s,
`crates/relay/src/os/session.rs:46`) bounds it.

Class: refactor

arch-1: none (C5, landed, shaped the three ends)

## SERV-9: The magic workspace label "reaped"

Kind: a magic string standing in for missing data.

Where:

- `reap_finished_sessions` (`service/mod.rs:2204`).
- `ReleasedTabs` (`workspace/mod.rs:494` to 499) and `WorkspaceStore::reap`
  (`workspace/mod.rs:1276`).
- `cleanup_released_tabs` (`service/mod.rs:2220`) and `Farewell::close_released`
  (`service/mod.rs:1022`).
- The recording index the label keys (`browser/adapter/chromium/protocol4/link.rs:285` to 330,
  read in `browser/adapter/chromium/protocol4/translate/records.rs`).

Problem: `reap_finished_sessions` passes "reaped" as the workspace of every browser command it
sends, because `ReleasedTabs` drops the id of the workspace it came from. The label only matters
where protocol4 keys its recording index by workspace, and there it names a workspace that does
not exist.

Proposed change: `ReleasedTabs` carries its `WorkspaceId`, and `cleanup_released_tabs` and
`close_released` lose their separate `&str`.

Ratings: payoff low-med, risk low, effort S. User-visible text: none. It touches another lane
(`workspace/`).

Verdict: partial. The placeholder is real (`service/mod.rs:2204`) and `ReleasedTabs` has no
workspace (`workspace/mod.rs:494-499`), but the label is inert: only the recording translators read
the link's workspace (`protocol4/link.rs:396`, `translate/records.rs`), and a reap sends only
`ClearDiagnostics` and `CloseTab` (`service/mod.rs:2244-2266`). Carrying a `WorkspaceId` is the
route C16 replaces: Decision 6.7 drops `BrowserPort::call`'s workspace parameter, which removes
"reaped" and "released" with nothing to carry. And `workspace/` is core's lane, like `service/`
(`docs/tasks/arch-1/BOOTSTRAP.md:32`), not another lane.

Class: refactor

arch-1: C16 (todo, gate none) covers it, by another route.

## SERV-10: How ServiceHost is built

Kind: structure (a constructor chain, a long function, a duplicate handle).

Where:

- The six constructors: `start` (`service/mod.rs:219`), `start_in_world` (227), `start_waiting`
  (263), `start_with_token` (281), `start_with_world` (298) and `start_with_diagnostics` (330 to
  491). The one production call is at `cli/desktop.rs:97`.
- In `start_with_diagnostics`: the latch reads (340, 347), the three `refused_by_quit_latch` calls
  (340, 347, 350), the housekeeping (353 to 356) and the audit thread (441).
- The listener thread's `.expect` (`service/mod.rs:1128`).
- `Intake.browser` and `Intake.browser_port` (`service/mod.rs:1055` to 1056), `Farewell.browser`
  and `Farewell.port` (849, 853), both made from one `Arc` (361 to 363).

Problem:

- Each constructor adds one injected dependency to the next, and the only production entry point
  is named `start_waiting`.
- `start_with_diagnostics` is 162 lines. It holds the start gate (the elevation check, and the
  latch read before and after the lock wait), the start housekeeping (`remove_old_images_beside`,
  `register_missing_browsers`), and the wiring of about fifteen parts and five threads.
- The audit thread is an unnamed `std::thread::spawn`, and the listener thread uses `.expect`,
  where the other startup threads are named and return their spawn error.
- The same browser port is held twice under two names, in `Intake` and in `Farewell`.

Proposed change:

- One `ServiceHost::start(home, terms, World)`, where `World` holds the elevation probe, the
  presence world and the idle clock, with a system builder and test builders.
- The gate pulled out into `admit(layout, terms) -> Result<ServiceLock>`.
- One `Arc<RelayBrowserPort>`, coerced where a `dyn BrowserPort` is needed (the executor and the
  release path).

Ratings: payoff med, risk low, effort M. User-visible text: none.

Verdict: partial. The six constructors, the 162-line `start_with_diagnostics`, the unnamed audit
thread (`service/mod.rs:441`) and the `.expect` (`service/mod.rs:1128`) are as stated. The
duplicate port is a seam, not a copy: production makes both handles from one `Arc`
(`service/mod.rs:361-363`), but the test harness gives `browser_port` and `Farewell.port` a
`FakeBrowser` and keeps `browser` the relay port (`service/mod.rs:3194-3198`, `3226-3232`,
`3244-3245`), and 34 call sites use it (`serve_fake`, `serve_reading`, `serve_governed`,
`service/mod.rs:3136-3150`). One `Arc<RelayBrowserPort>` removes that seam unless those tests
move to the relay port with a protocol-4 fake extension. `World` and `admit` are safe.

Class: refactor

arch-1: none

## SERV-11: Overloaded names

Kind: naming.

Where:

- "session": `cli/session.rs` (the call's caller, ADR-0106), `cli/session_end.rs` (OS sign-out)
  and `intake/session.rs` (the queue vocabulary), beside `ghostlight_relay::os::session`.
- "desktop": `cli/desktop.rs` (open, serve, call and policy) against `intake/desktop.rs` (the
  shell's stream contract).
- "authority" for the service process at `service/mod.rs:183,205,623`, `cli/desktop.rs:291` and
  `cli/diagnostics.rs:4` to 5, and for the policy in `AuthoritySnapshot` (`service/mod.rs:1835`).
- The struct `service::Intake` (`service/mod.rs:1046`) against the module `crate::intake`.
- `language::workbench` imported as `words` (`cli/doctor.rs:26`, `cli/status.rs:11`) and as
  `stray_words` (`cli/setup.rs:46`, `cli/deployment.rs:16`).
- `AuthorityAlreadyRunning`'s message (`service/mod.rs:207`).

Problem:

- "Session" names three different things in three neighboring modules.
- `cli/desktop.rs` holds four commands, while `intake/desktop.rs` is a wire contract, so "desktop"
  names both a command module and the shell's stream.
- "Authority" means both the process and the policy in one file.
- The struct `Intake` makes `intake: &Arc<Intake>` read as the module.
- Doctor's and status's sentences come from a module named for the workbench, under two different
  aliases.
- `AuthorityAlreadyRunning`'s message ("another Ghostlight orchestrator already owns this
  runtime") adds "orchestrator" as a third name for the process.

Proposed change: `cli::caller` for `cli::session`, `cli::sign_out` for `cli::session_end`,
`cli::open` plus `cli::serve` for `cli::desktop`, `ServiceContext` for `service::Intake`,
`ServiceAlreadyRunning` for `AuthorityAlreadyRunning`, and "service" for the process in every doc
comment.

Ratings: payoff med, risk low, effort M. User-visible text: none. `AuthorityAlreadyRunning`'s
message is never shown: `record_start_failure` and `settle_serve` (`cli/desktop.rs:106` to 117
and 173 to 183) and the startup diagnostics (`service/mod.rs:324`) filter it out.

Verdict: confirmed. Every cited name and line holds, and `AuthorityAlreadyRunning`'s message is
filtered at all three exits (`cli/desktop.rs:106-117,173-183`, `service/mod.rs:324`). Outside
`cli/`, only `main.rs:11` names a module it renames. Renaming `AuthorityAlreadyRunning` crosses
core and install (`cli/desktop.rs:110,176`), and C19c rewrites the same doc comment, so the two
belong in one commit.

Class: refactor

arch-1: C19c covers only the stale `AuthorityAlreadyRunning` doc, and I10 (landed) made doctor's
line say "service"; the rest: none.

## SERV-12: The diagnostics record has no type across three crates

Kind: a vocabulary that crosses process boundaries, held only by string literals.

Where:

- The service sink's record in `write_record` (`crates/bridge/src/diagnostics.rs:475` to 483).
- The edge logs' record (`crates/relay/src/log.rs:128`).
- `run_show` (`cli/diagnostics.rs:175`, 134 lines), which reads `ts_ms` at 219, 241 and 256,
  `component` at 227, 271 and 301, and `op` at 232 and 284.

Problem: two writers in two crates build the record (`ts_ms`, `run_id`, `component`, `event`,
`level`, `op`, `detail`) with `json!`, and the reader in a third place reads it back by string.
Nothing ties the reader's keys to the writers'.

Proposed change: one serde `Record` type in `crates/bridge`, used by the service sink and by
`diagnostics show`. A change to `crates/relay` bumps `RELAY` under the connectors guard
(ADR-0203), so pin the relay writer with a round-trip test against that type instead of editing
it.

Ratings: payoff med, risk med (the written lines must stay byte-identical, field order included),
effort M. User-visible text: none if the bytes stay the same.

Verdict: partial. The two writers do not build one record. The service sink writes `ts_ms`,
`run_id`, `component`, `event`, `level`, `op` and `detail`
(`crates/bridge/src/diagnostics.rs:475-483`); the edge log writes `ts_ms`, `component`, `relay`,
`pid`, `event` and `detail`, with no `run_id`, `level` or `op` (`crates/relay/src/log.rs:127-134`).
`run_show` tolerates both through defaults, and `--json` prints each line whole
(`cli/diagnostics.rs:245-246`), so a typed decode that drops `relay` and `pid` would change that
output; one `Record` needs optional fields and both edge-only ones. The byte risk is real:
serde_json has no `preserve_order` here (no `indexmap` in its `Cargo.lock` entry), so `json!`
writes keys alphabetically and a struct must declare them in that order. Bridge has no `serde`
dependency (`crates/bridge/Cargo.toml:9-12`), so the type is a manifest change, and the round trip
lives in bridge, since relay depends on no workspace crate.

Class: refactor, persisted

arch-1: none (I12, landed, made `show` name each edge log from `relay::log` and left the record
untyped)

## SERV-13: setup.rs runs install and uninstall through one bool

Kind: structure (a boolean parameter and a long interleaved function).

Where:

- `install: bool` in `cli/setup.rs:81,155,339,681,817,873,922,977`.
- The callers `run_setup(true, ..)` and `run_setup(false, ..)` (`cli/mod.rs:295` to 296), and
  `run_setup(true, ..)` in `cli/doctor.rs:253`.
- `run_setup_steps` (`cli/setup.rs:155` to 332).
- Literal sentences at `cli/setup.rs:187` to 190, 310, 377, 379 and 826.
- The report builder in `cli/deployment.rs:71`.

Problem:

- One bool selects between two journeys through 8 functions, and the call sites read as
  `run_setup(true, ..)`.
- `run_setup_steps` is 178 lines. It interleaves side effects with `println!` across five
  subsystems (native hosts, the command path, user assets, desktop integration and the Windows
  entries) before the client steps.
- A few sentences are literals here, while the rest come from `language::setup` and
  `language::deployment`.
- `cli/deployment.rs` already shows the better shape: it collects `Line`s and prints them at the
  end.

Proposed change: a `Direction` enum in place of the bool, separate install and uninstall step
lists, and steps that return lines instead of printing them. Move the literal sentences to
`language::setup` with the same words.

Ratings: payoff med, risk med (tests pin the output and its order), effort M-L. User-visible
text: none if the words and their order are kept.

Verdict: partial. The bool, its 8 functions and 3 callers, the 178-line `run_setup_steps` and the
literals hold (one more: the bail at `cli/setup.rs:326-329`). The proposed home is wrong: Decision
8 puts install words in `language/deployment.rs`, whose doc says every sentence of the reports and
refusals lives there (`language/deployment.rs:1-9`); `language/setup.rs` holds the setup view and
the closing summary and is surface's lane. I10 moved the others and left these for a later slice
"the same way" (`docs/tasks/arch-1/LEDGER-install.md:260-263`). Steps that return lines must keep
the order when a later step fails after earlier lines were collected.

Class: refactor

arch-1: none (I10, landed, left these lines for a later slice)

## SERV-14: Missing and stale docs

Kind: docs.

Where:

- `lib.rs:1`.
- `cli/mod.rs:1` to 6.
- `intake/mod.rs:3` to 4, against `service::spawn_listener` (`service/mod.rs:1097`).
- The limits at `service/admission.rs:14` to 19 and `service/mod.rs:93,97`.
- `take(120)` at `cli/session.rs:38,168`.

Problem:

- `lib.rs` is one line ("Ghostlight's single mutable, domain-driven browser orchestrator.") and
  maps none of its 17 modules, including that the crate is also the command line, the installer
  and the workbench.
- The `cli/mod.rs` doc describes only `call`.
- `intake/mod.rs` says every peer reaches the service through the listener in `listener`, but the
  accept loop is `service::spawn_listener`; `intake/listener.rs` only judges openings.
- `SESSION_CONTROLS`, `SESSION_BYTES`, `SERVICE_BYTES`, `CONTROL_BYTES`, `HANDSHAKES`,
  `SESSIONS`, `DIAGNOSTIC_CLEAR_BATCH_SIZE` and `QUEUED_METADATA_BYTES` have no doc comment.
- The caller's declared key and process name are each cut to 120 characters by an unnamed
  literal, written twice.

Proposed change: a `lib.rs` doc that maps the modules to the architecture and states the
crate-name mapping below; a `cli/mod.rs` doc for the whole command line; an `intake/mod.rs`
sentence that matches where the accept loop is; one-line docs on the limits; a named constant for
120.

Ratings: payoff low-med, risk low, effort S. User-visible text: none.

Verdict: confirmed. `lib.rs` has a one-line doc over 17 module lines (`scratch` is test-only), the
cited constants have no doc, and `take(120)` is written twice (`cli/session.rs:38,168`). One
nuance: `intake/listener.rs` also writes the accept, refuse and control replies
(`intake/listener.rs:141-178`), not only the judgement.

Class: refactor

arch-1: none

## Crate name

The crate name confuses a little. One thing has three names: the directory `crates/orchestrator`,
the package `ghostlight-service` (library `ghostlight_service`), and "orchestrator" as the
diagnostics component (`crates/bridge/src/diagnostics.rs:78`), which also appears in log file
names. It also produces the stutter `ghostlight_service::service`.

Do not rename anything. The directory is referenced 224 times in 89 files, many of them ADRs that
must not be rewritten. The package name is the shipped binary, which the install layout
(`install/layout.rs:145`) and the caller walk's process detection (`cli/session.rs`) depend on.
Renaming the log component would change the log file names that `diagnostics show` reads. State
the mapping in the `lib.rs` doc instead (SERV-14).

## Overall

The small modules are in good shape. `service/lifetime.rs`, `idle.rs`, `presence.rs`, `shell.rs`,
`toolset.rs`, `latch.rs` and `hint.rs`, and `intake/listener.rs`, `opening.rs`, `mcp.rs` and
`desktop.rs`, have documented constants, injected clocks where time matters, and rules tested on
their own. The debt sits in two places: `service/mod.rs`, where every stream driver ended up, and
the command line, where `call`, `open` and `serve` spread across `cli/mod.rs` and
`cli/desktop.rs`, and `setup.rs` grew one long interleaved function. SERV-1 and SERV-2 are real
defects behind magic numbers, and both are small to fix. The one structural change that helps
most is SERV-3: it is a mechanical move, it makes SERV-7, SERV-8 and SERV-10 easy, and it lets
each stream role be reviewed on its own.

## Skeptic summary

Verdicts: 9 confirmed (SERV-1, 2, 3, 5, 6, 7, 8, 11 and 14), 5 partial (SERV-4, 9, 10, 12 and
13), 0 refuted. Both traced defects hold: SERV-1 needs a session of about 270 tabs and a last
token in a 107-byte band under the cap, and SERV-2 needs a quit that outlasts 10 s. No proposed
change is model-visible, frozen or locked; SERV-5 and SERV-12 touch diagnostics bytes.

What I would do first:

1. SERV-1. It is a defect in the shipped v2.0.0: after an update or a crash, a large session's
   edge is refused on every attempt and each request expires until the client restarts its
   server. The fix is small, service-only and a fix-up of C11: raise the opening cap above the
   token cap with a `const` tie, and test an opening that carries a full token. That also rescues
   the tokens 2.0.0 edges already hold, which shrinking the encoder would not.
2. SERV-2. A whole uninstall can print a false "did not stop in time" with advice that cannot
   help. One derived constant and a few `const` asserts fix it, and no words change.
3. SERV-5, which is C19c. It is decided, gate none and small; it gives a failed MCP session a
   diagnostics line and pairs the detached line with the attached one. Landing it, with C16,
   before SERV-3 lets SERV-3's pure moves follow without colliding with open core slices.
