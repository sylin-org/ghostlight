# arch-1: core-language ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Files:
`language/` apart from `policy.rs`, `deployment.rs`, `workbench.rs`, `setup.rs` and
`readiness.rs`, plus the catalog and result goldens. Machine: test-01 (CachyOS). A slice's
specification is its finding in [sweep-2026-10-20/language.md](sweep-2026-10-20/language.md),
read with its skeptic's corrections, or its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`,
`landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Parked on 2026-10-21. Every slice in this ledger has landed: core-language's own, and the
core-service slices moved here (SERV-6, SERV-12, BROW-2), with the `workspace_refusal` cleanup
test-03 asked for after C12b-2. Nothing is open and nothing is held. A fresh session takes what
the coordinator hands it next.

How this lane works: build in `.target-lane-arch-core-language`; before each commit run fmt,
workspace clippy (check its exit code, not a piped tail), `cargo test -p ghostlight-service` and
`scripts/check-repository-integrity.ps1` (and `cargo test --workspace` plus
`node --test tests/guards/connectors.test.mjs` when bridge, relay or a manifest changes); commit
by path with `-s`; `git pull --rebase origin dev`, retest when the rebase brought code, push; ask
the owning lane for a hold before a cross-lane edit and report when it lands; after a defect fix
or a milestone, deploy with `pwsh scripts/dev-loop.ps1 -TargetDirectory
.target-lane-arch-core-language` and run a fresh `claude -p --model haiku` agent on the desk bell.
A slice's hash is filled in by the next slice's commit, since a rebase before the push changes it.

## Slices

In order. "After" names what a slice waits for.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| L-D1 | fix(language): a wheel scroll may leave out its direction, and its amount is checked | - | LANG-3's skeptic note | landed (59c67621) |
| L-D2 | fix(language): a summary with its coverage note stays within its bound, to the character | - | LANG-6 and its note | landed (69e58321) |
| L-D3 | fix(language): one skipped history entry is counted in the singular | - | LANG-9's note | landed (09d78bd9) |
| L-D4 | fix(language): the handle-like text check matches real `diag_` and `invocation_` ids | - | LANG-2's note | landed (eff79366) |
| L-D5 | fix(language): an inline upload of exactly 5,000,000 bytes is accepted | L-D4 | WORK-1's note; one pub exact counter in `language/`, which core-work's WORK-1 then reads | landed (b5135a14) |
| C17 | refactor(language): one wait vocabulary and one coverage predicate | - | ADR-0203 C17, moved from core; LANG-3, WORK-5 and BROW-3's wait parts | landed (C17a df5b5923, C17b 1b693758) |
| C12b-1 | refactor(language): each refusal is typed where it is learned, and its code is written once (the language half) | C17 | ADR-0203 C12b, moved from core | landed (5c65614a) |
| LANG-4 | refactor(language): RAWX requirements are read from the directory, failing closed | - | LANG-4 | landed (5016587b) |
| LANG-1 | refactor(language): every bound and default has one home, read by the catalog and the decoder | C12b-1 | LANG-1; a test of every advertised default comes first | landed (test f54e2469, block 512ad4d1) |
| LANG-2 | refactor(language): one `HandleKind` names every handle prefix | L-D4 | LANG-2; core-work adopts it at the mint sites | landed (01cc55e6) |
| LANG-3 | refactor(language): the closed fields are enums decoded once | C17 | LANG-3, reopened by Decision 14; its uses in `work/` and `browser/` edit in the same commit | landed (d15f5ba5) |
| LANG-10 | refactor(language): `mod.rs` splits into its module root, operations and decoding, and the catalog mounts plainly | LANG-3 | LANG-10; the move lands alone | landed (move 4f692e1e, rest a6b8d686) |
| LANG-5 | refactor(language): one `Tool` table derives the tool lists | LANG-10 | LANG-5; the catalog golden's order stays | landed (1a114340, 99013200) |
| LANG-7 | refactor(language): repeated sentences and mappings are written once | - | LANG-7 | landed (ec0166d5) |
| LANG-9 | refactor(language): one phrase module counts, lists and bounds | L-D3 | LANG-9; no word changes | landed (6b5f619e) |
| LANG-8 | refactor(language): the composition sentences leave `Outcome` | LANG-9 | LANG-8; `StepNotStarted` stays, its sentence is a persisted audit summary | landed (0b6c3817) |
| LANG-11 | docs(language): the item docs match the code | - | LANG-11 | landed (df9e4cac) |
| LANG-12 | refactor(language): the small cleanups | - | LANG-12; `invites_control` stays while the window reads it | landed (fb359bf5) |
| LANG-12b | refactor(cli): the command line reads the machine, and language/ does no I/O | LANG-12 | LANG-12's environment part, given by the coordinator; cross-lane into `cli/` | landed (e91c5518) |
| C20 | test(language): the plugin skill names only catalog tools and fields | I9b | ADR-0203 C20, moved from core | landed (6f37117f) |
| SERV-6 | refactor(cli): the command line reads the service's own vocabulary | - | SERV-6 with its skeptic's correction, moved from core-service; cross-lane into `cli/` and `intake/mcp.rs` | landed (15b38451) |
| SERV-12 | refactor(diagnostics): one typed record, written by the service and read by `diagnostics show`, with the edge's line pinned | - | SERV-12 with its skeptic's correction, moved from core-service; cross-lane into `crates/bridge`, `cli/diagnostics.rs` and `crates/relay/src/log.rs` (coordinator's go) | landed (40e4ff71) |
| BROW-2 | refactor(browser): the dead recovery decisions go | test-03's C16 | BROW-2, moved from core-service; cross-lane into `browser/recovery.rs` and `work/mod.rs` | landed (4c20823b before its rebase) |

## Notes

- L-D1: the decoder fills a wheel scroll's missing direction with `SCROLL_DIRECTION_DEFAULT`, so
  `work/pointer.rs`'s `expect("language validated direction")` still holds and work/ is untouched.
  A wheel scroll accepts the advertised amount default, which a defaults-filling client sends, and
  refuses any other amount: ticks set how far a wheel scrolls. The wheel branch's required list
  drops `direction`; the union requires only what every branch requires, so the catalog golden
  is unchanged.
- L-D2: `outcome::with_note` joins a summary and one closed note within `SUMMARY_MAX_CHARS`,
  cutting the summary and never the note. The coverage qualifier and both storage notes use it,
  so a qualified summary is at most 500 characters and a storage note reaches 500, not 499 (one
  more character at the bound, model-visible). The storage notes are named constants
  (`CHILDREN_UNSAVED`, `HISTORY_UNSAVED`). When storage fails on a summary already at the bound,
  the storage note wins and can cut the coverage note's words; the result's `coverage` fact still
  carries them. `workbench/mod.rs` still spells the children note (surface, LANG-6's refactor).
- L-D3: one skipped entry reads "1 unreadable history entry was skipped." in the history and
  status detail, as the unconfirmed-receipts sentence already did. Person-facing, on purpose.
  LANG-9 later folds both hand-written plurals into the shared phrase helpers.
- L-D4: each handle prefix in `field_problem.rs` carries its own shape: eight lowercase base32
  characters after `target_`, `tab_`, `view_` and `image_`, 32 lowercase hex digits after
  `invocation_`, and `<counter>_<8 hex>` after `diag_` (`extension/lib/diagnostics.js`). The set
  of prefixes is unchanged; `snapshot_` and `browser_` stay out of page-text replacement until
  LANG-2's `HandleKind` decides each kind's shape.
- L-D5: `language::base64_decoded_len` counts the bytes standard base64 decodes to exactly
  (padding dropped), and the upload decoder sums it, so 5,000,000 bytes pass and 5,000,001 are
  refused. The executor's `decode_inline_files` decodes for real and already accepted the
  boundary. For core-work: `work/forms.rs` keeps its own `base64_decoded_len` until WORK-1 reads
  this one, and the screenshot path estimates the same way the decoder did
  (`work/reading.rs`, `data.len() / 4 * 3`, against `workspace/mod.rs`'s image ceiling).
- C17a: `language/wait.rs` holds `WaitCondition` (all nine, in catalog order, with
  `settles_by_default`, `settles` and `page`) and `PageCondition`, the seven the page evaluates,
  which `BrowserCommand::Observe` now carries (with `tests_absence` and `reads_whole_page`).
  `Wait.condition` and `Postcondition.condition` decode through serde hooks that keep the curated
  refusal words exactly, pinned through `decode`. The catalog builds the wait branches, the
  `visual_settle` description and the expect enum from the types; the golden is byte-identical,
  and `the_wait_catalog_is_unchanged` compares the wait schema without the update variable. A
  duration wait now reads "Waited 1 second on example.com." instead of "The condition was met
  ..." (model-visible on purpose); no result golden holds a wait. Gone: the literals in
  `work/forms.rs` (its three settle rules are `WaitCondition::settles`), `work/documents.rs` and
  the test fake, and the translator's `condition` module with its unknown-condition failure.
  Cross-lane edits, under the coordinator's hold: `work/forms.rs`, `work/documents.rs`,
  `work/mod.rs`, `work/tests/execution.rs` (two test matches), `browser/adapter/work.rs`,
  `protocol4/mod.rs`, `translate/observe.rs`, `translate/tests.rs` and `browser/mod.rs`. The
  duration re-parse in `work/forms.rs` stays for WORK-13's `finish_wait`.
- C17b: `Coverage::withholds` (a limit on the requested scope, or an exclusion elsewhere on the
  page) and `COVERAGE_FACT` live in `language/coverage.rs`. Composition reads a step's coverage
  from `terminal.audit.coverage()`, which the completion path sets with the fact, so its JSON
  parse is gone; the lean view deserializes the block and asks `withholds`, so its field-name
  list is gone (`a_new_limitation_field_reaches_the_lean_view`). One difference: a partial
  coverage object, which no result carries, now stays in the lean view instead of being dropped.
  Cross-lane: `work/composition.rs`, `work/receipt.rs`, `intake/mcp/lean.rs`.
- C12b-1: proposed to the coordinator on 2026-10-20 as the language half: `Refusal::reason()`, the
  closed causes (`FileProblem`, a closed export word, `ReasonCode` on `AuthorityBlocked` and
  beside `ActionUnconfirmed`'s persisted `CheckFailure`, a `PrimitiveCause`), the (model code,
  audit tag) table test, and work's constructor sites passing the cause they already know, with
  every fact unchanged. C12b-2 makes the result write `reason()` and deletes the literals; the
  adapter maps Decision 5.2's receipt codes onto the causes.
- LANG-4: each directory entry is a named constant, and `requirements()` reads the entry an
  exhaustive match over `Operation` reaches, so a requirement set is written once and no
  operation can miss the directory (it fails closed by construction, not by a runtime default).
  `DIRECTORY` keeps its order, so `ghostlight policy` prints the same table. The test fixture
  now checks that each entry's fixture reaches that entry, and that no (tool, variant) repeats.
  `work/tests/catalog_authority.rs` still keys requirements by (tool, variant); that copy is
  core-work's.
- C12b-1, as confirmed by the coordinator: `Refusal::reason()` is every variant's closed model
  code. Varying codes became closed causes: `AuthorityBlocked { code: ReasonCode }` (its
  `BlockedReason` is derived by `BlockedReason::of`, which replaces work's `blocked_reason`, and
  durable history still keeps the group), `LandingDeniedUnknown { code }`, `ActionUnconfirmed {
  code }` beside its persisted `CheckFailure`, `FilesUnreadable { problem: FileProblem }` and
  `BrowserPrimitive { cause: PrimitiveCause }` (`Refused` today; `DebuggerUnavailable` and
  `SessionLost` have their own sentences and steps for the adapter to map, Decision 5.2).
  `every_refusal_has_one_model_code_and_one_audit_tag` pins both words for every variant and
  cause. A temporary probe in work's builders showed that every refusal the suite reaches
  writes exactly `reason()`, apart from the prose of the recording export. For C12b-2:
  `CredentialAuthorization` (`credential_authorization_required`) and `NoDialogVisible`
  (`no_dialog_visible`) write no reason fact today, and `RecordingExportFailed` and
  `RecordingNotAttached` write the browser's bounded text, so making the result write
  `reason()` changes those facts (and moves that text to `detail`). `FilesUnreadable` already
  writes its closed code: model-visible here for an inline file that is not base64
  (`inline_file_not_base64`), over the ceiling (`upload_too_large`) and a stale capture
  (`capture_stale`), which wrote prose; no golden holds them. Cross-lane test edits:
  `governance/tests.rs` and `audit/record_tests.rs` (two `AuthorityBlocked` literals), and
  `work/navigation.rs` (the landing code).
- LANG-1, first commit: `every_advertised_default_is_the_decoded_default` decodes every
  known-good call and catalog example with its advertised defaults left out and filled in, and
  needs the same operation, so a decoder default that drifts from the catalog fails (checked by
  changing `find`'s scope default). `visual_settle` and the scroll direction and amount stay
  unset at decode; the test checks settlement reads the same and the scroll defaults are the
  shared constants. No default had drifted.
- LANG-1, second commit: `language/bounds.rs` holds every bound and default the decoder checks
  and the catalog advertises (a `Range` for each integer field, the character limits, the
  timeouts, the string defaults until LANG-3 makes those fields enums). The decoder validates
  with `validate_within`, the catalog builds integer fields with `within`, and the sentences
  that name a bound (fill fields, flow steps, duration, zoom, ticks, url) format it. The catalog
  golden is byte-identical. The skeptic's traps: the url bound counts bytes in the decoder and
  characters in the schema, documented on `URL_MAX`; the coordinate bound is an integer so the
  schema still prints `1000000`. The 5,000,000-byte inline ceiling stays where C12b left it,
  and `COMPOSITION_STEP_LIMIT` stays in `history.rs` for LANG-10 to move.
- LANG-2: `language::HandleKind` names the eight kinds (tab, target, view, image, snapshot,
  browser, diagnostics cursor, invocation) with each prefix and the shape Ghostlight mints
  (`minted`). The decoder's 44 prefix literals and the catalog's `handle()` take the kind; the
  page-text replacement and the hints self-check read `HandleKind::IN_PAGE_TEXT`, the set they
  already used, so no word changes; the golden helper reads the target prefix and
  `WORKSPACE_SUFFIX_CHARS`. A test pins the extension's `browser_` and `diag_` spellings. The
  length bound stays `bounds::HANDLE_CHARS`. Left to their lanes: the five mint sites in
  `workspace/mod.rs` (core-work) and `BROWSER_ID_PREFIX` in `browser/mod.rs` (core-service).
- LANG-11: `Outcome::next_steps` and `Refusal::next_steps` say at most two steps, and the
  composition docs in `outcome.rs` and `language/audit.rs` say flow, not "flow or sequence".
  `WorkspaceReason::next_steps` does return one step at most, so its doc stays. The third
  comment, `setup.rs:10`'s "four stages", is in the surface lane's `language/setup.rs`; it was
  handed to leo-desktop-02.
- LANG-7: the view and resize refusals are `NEEDS_VIEW` and `RESIZE_NEEDS_SIZE`, the
  value-not-accepted refusal is `refuses_value`, the 28 `place(host, "the page")` calls are
  `on_page(host)`, and `channel_off` reads `channel_switch::work_from` (renamed from the private
  `from`). The test that pins the view sentence keeps its literal, as the oracle. No word changes.
- LANG-9: `language/phrase.rs` holds `counted`, `counted_or_none`, `article`,
  `one_line_bounded`, the digit grouping and the one `NO_VALUE`, moved out of `outcome.rs`
  unchanged; `field_echo`, `field_problem` and `audit` read it. Every language module can, so
  surface's `tabs_count` and `join_list` and install's `either_of` can move onto it in their
  lanes. The hand-written plurals in `composition.rs`, `coverage.rs` and `audit_health.rs` stay:
  `counted` groups digits from 1,000, so adopting it there is a visible choice, not this slice.
- LANG-12: `object()` no longer forwards to `raw_object()` (one function), and
  `page_recovered_summary` and `tab_already_closed_summary` are private to `outcome.rs`.
  `invites_control` stays, as the row says. Not taken here: moving `EnvironmentFacts::observe`
  and `current()` out of language/. Its callers are `cli/doctor.rs` and `cli/setup.rs` (install
  lane), and it needs a home outside language/; raised with the coordinator.
- LANG-8: the live-history sentences of a running flow and of a restored one with no
  completion record are `composition::running_summary` and `composition::UNRECORDED_SUMMARY`,
  so `Outcome::CompositionProgress` (which shared its name with the struct) and
  `Outcome::CompositionUnrecorded` are gone. `StepNotStarted` stays, the typed author of the
  persisted not-started line. Cross-lane: `workbench/history.rs`, two lines. No word changes.
- LANG-3: `language/choices.rs` defines the closed choices with one `closed_choice!` macro
  (enum, `WORDS` in catalog order, `as_str`, and a `Deserialize` that refuses with the curated
  "`field` must be one of ...: got `value`" words, pinned through `decode`): `InspectScope`,
  `FindScope`, `PointerButton`, `ScrollDirection`, `ScrollAmount`, `HistoryDirection`,
  `DiagnosticSource`, `DiagnosticDetail`, `OnError`, `DialogAction` and `RecordAction`. The
  decoder's `validate_choice` calls for them go; the catalog advertises `WORDS` (golden
  byte-identical); the defaults are typed; `HISTORY_FORWARD` became `HistoryDirection` and
  `HISTORY_RELOAD`. Work matches the types: the diagnose fallback `_ => Both` and the
  recording `unreachable!` are gone, and `Outcome::PageScrolled` carries a `ScrollDirection`, so
  pointer's literal "down" is gone. `BrowserCommand`'s fields stay strings, filled with
  `as_str()`; test-02's BROW-3a retypes them. Cross-lane, under test-03's hold: `work/flow.rs`,
  `interaction.rs`, `mod.rs`, `navigation.rs`, `pointer.rs`, `reading.rs`, `recording.rs`. Left:
  `Outcome::HistoryTraversed` still carries its direction as a String, and the modifiers stay
  strings (BROW-3).
- LANG-12b: `cli/machine.rs` (now the install lane's file) reads the environment facts and
  resolves the row (`machine::environment()`); `language/environment.rs` keeps
  `EnvironmentFacts`, `Environment` and `resolve` and does no machine I/O. The three call sites
  in `cli/doctor.rs` and `cli/setup.rs` follow, under test-04's hold.
- LANG-10, the move: `language/mod.rs` (3,255 lines) splits into the module root (808: the
  module list, re-exports, `LanguageError`, the instructions, the role and key vocabularies and
  the tests), `operation.rs` (the request types and their serde defaults) and `decode.rs`
  (decoding and validation). Every original line is in one of the three, checked by line
  multiset; the only new lines are the two module docs, `use super::*;` and the re-exports.
- LANG-10, the rest: the catalog mounts as a plain `mod catalog;` beside the re-exported
  `catalog::catalog` (a module and a function live in different namespaces), so the
  `tool_catalog` alias is gone; the root doc names the model-facing and the person-facing
  modules; `COMPOSITION_STEP_LIMIT` moves from `history.rs` to `composition.rs`, with its readers
  in decode, the catalog, the audit and `workbench/history.rs` (surface, one import line).
- LANG-5: `language::Tool` lists the 23 tools in catalog order with each name, title,
  settlement flag and default call time. The catalog builds each tool from it (golden
  byte-identical), `EXPECTED_TOOL_NAMES` and `CATALOG_TOOL_COUNT` derive from `Tool::ALL`,
  `Operation::tool()` maps every operation (and `name()` reads it), the decoder dispatches on
  `Tool::named` with an exhaustive match, settlement's `PAGE_TOOLS` gives way to
  `Tool::settles`, and the capability directory, `CompositionKind::tool` and the flow's
  forbidden list name tools through it. The directory keeps its own order, so `ghostlight
  policy` prints the same table. The serde timeout defaults per struct stay, held equal to
  `Tool::default_timeout_ms` by the existing test.
- LANG-5's follow-up (57737b58): C10c's private `SCREENSHOT_TOOL` reads `Tool::Screenshot`.
- C20: `the_plugin_skill_names_only_catalog_tools_and_fields` reads the plugin skill's body after
  its YAML front matter through the hints checker and needs no violation; a field the catalog
  does not advertise, appended, must fail it. The body passes as I9b left it.
- SERV-6: the command line reads the engine's `JSONRPC_VERSION`, `content::IMAGE` and the new
  `result_field` keys (`content`, `structuredContent`, `isError`, which the engine now writes
  through them) from `intake/mcp.rs`, instead of spelling them again; `exit_code` decodes
  `work::result::Status` and matches it exhaustively, a word this build does not know still
  exiting as unknown; `SUPPORTED_REVISIONS` names `STRUCTURED_CONTENT_REVISION`. Cross-lane, under
  test-04's hold: `cli/client.rs` and `cli/mod.rs`; `work/result.rs` needed no edit.
- SERV-12: `ghostlight_bridge::diagnostics::Record` is the one diagnostics line, its fields in
  alphabetical order (serde_json writes `json!` keys that way) with the sink-only `run_id`,
  `level`, `op` and the edge-only `relay`, `pid` optional; the sink writes through it
  (`a_sink_record_writes_the_bytes_the_sink_always_wrote`), and `diagnostics show` decodes it
  while `--json` still prints each line as written. `ghostlight_relay::log::line` is now the one
  place the edge builds its line (bytes unchanged, the coordinator's go, revision 5 unpublished),
  and `an_edge_line_reads_as_a_record` pins it. On test-01's real diagnostics directory the old
  and new `diagnostics show`, plain and `--json`, printed identical output (304 and 2,589 lines).
  `crates/bridge` gains its `serde` dependency; the connectors guard passes.
- BROW-2: `RecoveryDecision::Launch`, which nothing constructed, is gone with the two
  `unreachable!` arms that matched it in `work/mod.rs`. Recovery's `decide`, `decide_browser` and
  `start_plan` return the `FlightPhase` directly, so `RecoveryPlan` and `phase_from_plan` (whose
  both-false arm was unreachable) go; `starts_on_request` matches the launching phases. `From<
  RecoveryWaitError> for BrowserError` replaces the mapping `work/mod.rs` wrote twice.
- The `workspace_refusal` cleanup: `language::outcome::workspace_refusal` returns the `Refusal`
  alone. Since C12b-2 the result writes `refusal.reason()`, so its second element, the reason
  word, had no reader; the test now checks `reason()` for every workspace error instead.
