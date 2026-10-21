# Second sweep, 2026-10-20: language

This sweep read `crates/orchestrator/src/language/`: 33 files, about 26,000 lines. It was read-only,
on `dev` at `8f6a43ec`. Production code was read in full and tests were skimmed. Where a finding
crosses into `work/`, `workspace/` or the browser adapter, that code was read too. Nothing was
edited, built or run. The catalog golden (`docs/1.0/catalog.json`) and the result goldens
(`tests/golden/model/*.json`) pin model-facing text. The unit tests
`outcome_summaries_transcribe_the_product_oracles` and `refusal_wording_and_recovery_stay_exact`
pin many more sentences exactly. A change to model-visible words is a deliberate model-surface
change, not a refactor, so every section rates its effect on model text on its own line. Where a
finding meets a decision of ADR-0203, the section says so. Findings are ranked by payoff over risk.

## LANG-1: Every bound and default is written twice, in the decoder and in the catalog

Kind: magic literals, two sources for one fact.

Where:

- The decoder checks bounds with bare literals: `crates/orchestrator/src/language/mod.rs:1017`
  (`max_chars` 500 to 50_000), `1043` (`max_depth` 1 to 12), `1071` (`max_results` 1 to 50),
  `1227` (`max_result_chars` 100 to 20_000), `1418` (zoom 25 to 500), `1438` and `1439` (window
  320 to 7_680 and 240 to 4_320), `2030` (fill fields 1 to 30), `2049` (field value 8_000),
  `2207` to `2222` (upload paths 5, path 4_096, file name 255, media type 100), `2250`
  (coordinate 1_000_000), `2390` (wait value 2_000) and `2472` (handle length 160).
- The decoder's defaults are literals too: the `default_*` functions at `mod.rs:2537` to `2578`
  (8_000, "controls", "any", 80, 20, "primary", 1, 1, "both", "problems", 50) and
  `default_on_error` "stop" at `mod.rs:891`.
- The catalog writes the same numbers again through `integer()`, `text()`, `handle()` and
  `coordinate()`: `crates/orchestrator/src/language/catalog.rs:417`, `428`, `461`, `490`, `499`,
  `518`, `580`, `716`, `977` to `981`, `1022`, `1140`, `1553` and `1590`.
- The description text restates them as well. "Run 1 to 20 tool calls" (`catalog.rs:186`) sits
  beside the `COMPOSITION_STEP_LIMIT` that bounds the same array. "Fill 1 to 30 fields"
  (`catalog.rs:130`) and "milliseconds from 0 to 10000" (`catalog.rs:910`) do the same. Decoder
  messages spell "25 to 500" (`mod.rs:1417`), "1 to 10" (`mod.rs:2010`), "5,000,000-byte"
  (`mod.rs:2240`, beside `UPLOAD_AGGREGATE_BYTES`) and "4096 bytes" (`mod.rs:2418`).
- Tests pin the decoder defaults a third time (`mod.rs:2912`, `2921`, `2925`, `2936` and `2937`).

Problem:

- The constants are only half applied. `MAX_INSPECT_ITEMS` and `SCRIPT_MAX_CHARS` are shared, yet
  the `max_items` default 80 is still a literal on both sides (`catalog.rs:499`, `mod.rs:2555`).
- `validate_condition` checks the wait value with a literal 2_000 (`mod.rs:2390`), where
  `validate_expect` uses `MAX_POSTCONDITION_VALUE_CHARS` for the same bound (`mod.rs:1840`).
- `INLINE_FILE_LIMIT` (`mod.rs:2141`) is used only by the decoder. The catalog writes 5
  (`catalog.rs:981`).
- The per-tool timeout defaults live in two places: `timeout(tool)` matches tool names
  (`catalog.rs:1572` to `1579`), and the decoder sets them per struct with serde attributes
  (`mod.rs:683`, `918`). The test at `mod.rs:2625` exists to keep the two in step.
- A hidden hazard: tests check only that an advertised default is accepted
  (`crates/orchestrator/src/language/self_check/catalog.rs:379`). Only `timeout_ms` is checked for
  equality (`mod.rs:2625`). A `find.scope` default that drifted between the catalog and the
  decoder would pass every test.

Proposed change: one block of named bounds and defaults beside the decoder. The decoder's checks,
the catalog's schema helpers and the `format!` calls in descriptions all read it.

Ratings: payoff high, risk low, effort M. Model text: none if the values are kept.

Verdict: confirmed. Every cited literal is at its line, and no test compares a decoded default
with the advertised one except `timeout_ms`: nothing reads the decoded `find` scope, and
`protocol4/translate/tests.rs:377` builds its "any" by hand. Two traps for the block: the url
bound counts bytes in the decoder (`mod.rs:2416`) and characters in the schema
(`catalog.rs:1568`), and a plain `format!` of 5_000_000 drops the commas the message prints
(`mod.rs:2240`). That ceiling is also spelled at `work/forms.rs:1381`, `work/mod.rs:3005` and
`workspace/mod.rs:2274`.

Class: refactor.

arch-1: C12b owns the 5,000,000-byte inline ceiling (Decision 6.4: one constant, checked once at
intake). The other bounds and defaults: none.

## LANG-2: Handle prefixes are about 75 bare literals

Kind: vocabulary that leaves its module.

Where:

- 44 prefix literals in the `mod.rs` validators ("tab_" 22 times, "target_" 14, "view_" 5,
  "image_", "diag_" and "browser_" once each).
- 27 in the catalog's `handle()` calls in `catalog.rs`.
- 5 mint sites in `crates/orchestrator/src/workspace/mod.rs:611`, `2115`, `2224`, `2288` and
  `2469`.
- `BROWSER_ID_PREFIX` already exists (`crates/orchestrator/src/browser/mod.rs:39`), but the
  language module spells "browser_" again.
- `HANDLE_PREFIXES` is copied in `crates/orchestrator/src/language/field_problem.rs:32` and
  `crates/orchestrator/src/language/self_check/hints.rs:53`.
- The 8-character suffix is restated in four constants: `workspace/mod.rs:30`,
  `field_problem.rs:34`, `self_check/hints.rs:56` and `self_check/golden.rs:51`. Two of them are
  test-only.
- The 160-character handle limit is in `mod.rs:2472` and `catalog.rs:1553`.
- "diag_" cursors are minted by the extension (`extension/lib/diagnostics.js:235`), so that prefix
  crosses a language boundary as well.

Problem: the prefixes are one vocabulary. They are minted in `workspace/`, checked in the
decoder, advertised as patterns in the catalog, and recognized in field problems and hints. Each
place spells them on its own.

Proposed change: one `HandleKind` (prefix, ALL, suffix length, max length) where handles are
minted. The validators and `handle()` take the kind. A test can pin the extension's "diag_"
spelling.

Ratings: payoff high, risk low, effort S-M. Model text: none.

Verdict: confirmed. The counts hold: 44 prefix literals in `mod.rs` before its tests, 27
`handle()` calls and five mint sites. The copies also disagree, which a `HandleKind` must carry:
`snapshot_` is minted (`workspace/mod.rs:2224`) but in no list; `HANDLE_PREFIXES` holds
`invocation_` (32 hex digits, `work/invocation_id.rs:14`) and `diag_` (`diag_<n>_<8 hex>`,
`extension/lib/diagnostics.js:25`), whose shapes its 8-character test never matches
(`field_problem.rs:888-899`); and `browser_` ids are minted by the extension
(`extension/lib/state.js:32`), which also bounds a cursor at 160 (`diagnostics.js:16`). So each
kind needs its own shape, and page-text redaction must keep its own set to stay model-neutral.

Class: refactor.

arch-1: none.

## LANG-3: Closed vocabularies decode to String and are parsed again downstream

Kind: vocabulary and structure.

Where:

- `Wait.condition` is a `String` (`mod.rs:835`). It is matched again in
  `crates/orchestrator/src/language/outcome.rs:2405` (`waited()`, with an unreachable `_` arm at
  `2423`), in `crates/orchestrator/src/work/forms.rs:801`, `899` and `1043`, in
  `crates/orchestrator/src/work/documents.rs:137` and in `browser/mod.rs:1672`. It also has a
  second set of constants in `crates/orchestrator/src/browser/adapter/chromium/protocol4/translate/observe.rs:32`
  to `48`.
- `Diagnose.source` and `Diagnose.detail` are parsed again in
  `crates/orchestrator/src/work/interaction.rs:156` to `165`, with a silent `_ => Both` fallback.
- `Record.action` and `HandleDialog.action` are matched as strings in
  `crates/orchestrator/src/language/capability_map.rs:307` to `311`,
  `crates/orchestrator/src/work/recording.rs:46`, `work/interaction.rs:48` and `95`, and
  `crates/orchestrator/src/work/mod.rs:2621`.
- `RunFlow.on_error` is compared to "stop" in `crates/orchestrator/src/work/flow.rs:80` and `121`.
- `InspectPage.scope` is compared to "document" in `crates/orchestrator/src/work/reading.rs:91`.
- `ScrollPage.direction` and `amount` (`mod.rs:585` to `587`), `Click.button` (`mod.rs:562`) and
  `Find.scope` (`mod.rs:487`) are strings as well.
- A duration is parsed and checked by the decoder, then thrown away (`mod.rs:2362` to `2370`). The
  executor parses it again, with `unwrap_or(0)` (`work/forms.rs:805` to `809`).
- `HISTORY_FORWARD`'s doc says the catalog and the decoder use it (`mod.rs:1366`). The decoder and
  `crates/orchestrator/src/work/navigation.rs:562` do, but `catalog.rs:387` writes "forward" as a
  literal.

Problem: each value is checked once as a string and then re-read as a string in every consumer.
A consumer that falls back to a default hides a decoder mistake instead of failing to compile.
AGENTS.md asks for a vocabulary that leaves its module to live in one domain module, as an enum
with its rendering.

Proposed change:

- Use enums with ALL and `as_str()`, as `ReadMode`, `ReusePolicy` and `area::PageArea` already do.
- `validate_choice` and the catalog's `enumeration()` read ALL.
- `WaitCondition::Duration(ms)` carries the parsed milliseconds, and `settles_by_default()` lives
  on the type.
- Defaults applied at decode also make an omitted field and a filled-in default decode to the same
  `Operation`.

Prior decision: ADR-0203 Decision 8 already decides one closed `language::WaitCondition`. Slice
C17 carries it, and `docs/tasks/arch-1/LEDGER-core.md` lists C17 as `todo`. C17 removes the
condition literals in `work/forms.rs` and `work/documents.rs`, and the second condition module in
`translate/observe.rs`. Its text does not say whether `WaitCondition` carries the parsed
milliseconds. If it does, the re-parse at `work/forms.rs:805` goes with it. The same ADR lists
"Closed enums for `on_error`, `Record.action`, `HandleDialog.action` and Diagnose's fields" under
Considered and not taken, because they fix no defect a person or agent meets. This finding does
not reopen that decision. The silent `_ => Both` fallback and the half-applied `HISTORY_FORWARD`
are new evidence for whoever next touches that code. Reopening the declined part needs an ADR-0203
amendment.

Ratings: payoff high, risk med (it touches `work/` and the adapter), effort M-L. Model text: none,
as long as `validate_choice` keeps producing the refusal and serde's "unknown variant" errors never
reach the model. Rating note: most of the payoff is the wait part, which C17 already owns. Judged
by ADR-0203's own test, the declined part alone has low payoff.

Verdict: partial. Corrections: the `_` arm of `waited()` (`outcome.rs:2423`) is reachable. A
duration wait builds `Outcome::Waited` with condition "duration" (`work/forms.rs:864-869`) and
reads "The condition was met", the sentence C17's `a_duration_wait_has_its_own_sentence`
replaces. `browser/mod.rs:1672` is in the test-only `testing` fake (`browser/mod.rs:1471-1786`).
Of the precedents only `PageArea` has `ALL` (`area.rs:37`, with `word()`); `ReadMode` has
`as_str()` and no `ALL`, `ReusePolicy` has neither, and `ReadMode` decodes through serde, so a bad
`mode` already returns serde's unknown-variant text (`mod.rs:427`, `1598`). A decode-time default
for `ScrollPage.direction` is a behavior change: a wheel scroll that omits it is refused today
(`mod.rs:1989-1992`), though the schema advertises "down" (`catalog.rs:656-660`). New: a wheel
scroll never checks `amount` (`mod.rs:1988-2016`), so any string decodes and is ignored.

Class: refactor, model-visible (C17's duration sentence; a scroll default applied at decode).

arch-1: C17 (Decision 8, wait conditions). The `on_error`, `Record.action`, `HandleDialog.action`
and Diagnose parts would reopen "Closed enums for `on_error`, `Record.action`,
`HandleDialog.action` and Diagnose's fields". The scroll, button and scope parts: none.

## LANG-4: RAWX requirements are written twice

Kind: structure, two sources for one fact.

Where:

- `DIRECTORY` (`capability_map.rs:39` to `262`) states each tool variant's requirement set.
- `requirements()` (`capability_map.rs:271` to `313`) states every set again, by `Operation`
  variant.
- A third table, the test fixture (`capability_map.rs:333` to `404`), keeps the two in step.
- The module doc (`capability_map.rs:1`) says "One authoritative RAWX requirement directory".

Problem: a requirement set is an authorization fact, and it has two homes. A new variant needs
both tables and the fixture.

Proposed change: give `Operation` a variant key, and make `requirements()` a lookup in
`DIRECTORY`. The fixture test then only checks coverage.

Ratings: payoff med-high (these are authorization facts), risk low, effort S-M. Model text: none.
Renaming the opaque variant keys "inspect" (record status, stop and discard), "resolve" (dialog
accept, dismiss and respond) and "save_client" (which also covers download) would change words a
person reads in `ghostlight policy`, which prints them (`crates/orchestrator/src/language/policy.rs:256`).

Verdict: confirmed. Both tables and the fixture are as cited, and `policy.rs:256` prints the keys.
There is a fourth copy: `work/tests/catalog_authority.rs` keys `Case.required` by the same (tool,
variant) pairs and checks DIRECTORY against the executor. The lookup has to be total: a key that
misses the table must fail closed, never read as `CapabilitySet::EMPTY`, so a typed key that
indexes DIRECTORY is safer than a string search.

Class: refactor.

arch-1: none.

## LANG-5: One tool's facts live in about ten tables keyed by name

Kind: structure, derive from one source.

Where:

- `EXPECTED_TOOL_NAMES` (`catalog.rs:17`) and `catalog()` (`catalog.rs:48`).
- `timeout(tool)` (`catalog.rs:1572`), against the per-struct serde timeout defaults
  (`mod.rs:683`, `918`).
- `Operation::name` (`mod.rs:184`).
- The `decode` match, with a hand-kept list of allowed fields per tool (`mod.rs:993` to `1251`).
- `decode_flow_step` and its per-tool tab rules (`mod.rs:1702`).
- `FLOW_FORBIDDEN_TOOLS` (`mod.rs:1627`) and `RETIRED_TOOLS` (`mod.rs:977`).
- `PAGE_TOOLS` and `enabled()` in `crates/orchestrator/src/language/settlement.rs:6` and `26`.
- `DIRECTORY` in `capability_map.rs`.
- `CompositionKind::tool()` (`crates/orchestrator/src/language/history.rs:283`).
- Test copies of the full name list: `capability_map.rs:408` and `catalog.rs:1693`.

Problem: "is this a tool?" is answered three ways: by `decode`, by `EXPECTED_TOOL_NAMES`, and by
`DIRECTORY` (`mod.rs:1753`, `crates/orchestrator/src/language/audit.rs:457`). Adding a tool means
touching every table, and tests catch a missed one only where someone wrote a cross-check.

Proposed change: one closed `Tool` enum, with ALL in catalog order and the name, title, settlement
flag and default timeout of each tool. `Operation` maps to it, and the lists above derive from it.

Ratings: payoff high, risk med (broad, but compile-checked, and the catalog golden pins the order),
effort L. Model text: none.

Verdict: partial. The tables are where cited. Correction: `EXPECTED_TOOL_NAMES` and
`CATALOG_TOOL_COUNT` are read only by tests (`catalog.rs:1613-1615`, `mod.rs:2749-2760`), so
production answers "is this a tool?" two ways, `decode`'s match (`mod.rs:993-1250`) and DIRECTORY
(`mod.rs:1753`, `language/audit.rs:457`), beside `RETIRED_TOOLS`. DIRECTORY's tool order also
differs from the catalog's (upload, execute, wait, flow, dialog against wait, dialog, upload,
execute, flow), and `ghostlight policy` prints DIRECTORY in its own order (`policy.rs:256`), so a
list derived from a catalog-order `ALL` would reorder what a person reads there.

Class: refactor.

arch-1: none.

## LANG-6: Hand-worked truncation bounds

Kind: magic numbers.

Where:

- `crates/orchestrator/src/language/audit_health.rs:24` takes 461 characters and
  `audit_health.rs:38` takes 471.
- "At most two next steps" is a literal in `crates/orchestrator/src/work/result.rs:154` and
  `outcome.rs:1049`, and a private `NEXT_STEPS_MAX` in `field_problem.rs:48`.
- `crates/orchestrator/src/audit.rs:176` takes 500 characters as a literal, outside the language
  module, instead of `SUMMARY_MAX_CHARS`.

Problem: 461 and 471 are 500 minus the length of the appended suffix (38 and 28 characters), minus
one more. The qualified summary tops out at 499, so the bound is off by one. Nothing ties the
numbers to `SUMMARY_MAX_CHARS` or to the suffix text.

Proposed change: name the two suffixes, and derive both bounds from `SUMMARY_MAX_CHARS` and the
suffix length. Publish `NEXT_STEPS_MAX` beside `SUMMARY_MAX_CHARS`.

Ratings: payoff med, risk low, effort S. Model text: at most one more character, at the bound only.

Verdict: confirmed. The suffixes are 38 and 28 characters with their leading space, so both paths
stop at 499. Two facts for the fix: the children suffix is spelled again at
`workbench/mod.rs:1987`, and the coverage qualifier (`coverage.rs:79-86`, applied at
`work/receipt.rs:35-37`) appends up to 102 characters with no bound, so a saved result's summary
can pass 500, and `take(471)` can cut the coverage words when storage fails. Pointing
`audit.rs:176` at `SUMMARY_MAX_CHARS` would tie the audit line's bound to the model's; a named
audit constant keeps the two apart.

Class: refactor, model-visible (one character at the bound).

arch-1: none.

## LANG-7: Repeated sentences and mappings

Kind: duplication.

Where:

- "region work needs a current view handle: take a screenshot, then pass its view with these
  coordinates" appears 4 times (`mod.rs:1891`, `1998`, `2121` and `2185`).
- "action resize needs width and height: the outer window size in pixels" appears twice
  (`mod.rs:1428`, `1434`).
- "{condition} does not accept value" appears twice (`mod.rs:2379`, `2405`).
- `place(host, "the page")` appears 28 times in `outcome.rs`.
- `channel_off` (`outcome.rs:2389`) maps `IntakeChannel` to "MCP agents" and "the command line",
  as `from()` already does in `crates/orchestrator/src/language/channel_switch.rs:156`.

Problem: the same model-facing words are typed several times, so one copy can change without the
others.

Proposed change: named constants, an `on_page(host)` helper, and reuse of `from()`.

Ratings: payoff med, risk low, effort S. Model text: none.

Verdict: confirmed. The view sentence has four production copies (`mod.rs:1891`, `1998`, `2121`,
`2185`) and a test pin (`mod.rs:3044`); the resize and "does not accept value" sentences have two
each; `place(host, "the page")` is called 28 times; and `channel_off` repeats the words of
`from()` (`channel_switch.rs:156`), which is private and must be opened to `outcome`.

Class: refactor.

arch-1: none.

## LANG-8: Outcome variants that exist only to borrow a sentence

Kind: naming and structure.

Where:

- `Outcome::CompositionProgress` and `Outcome::CompositionUnrecorded` (`outcome.rs:417` and
  `419`) are built only by `crates/orchestrator/src/workbench/history.rs:114` to `120`.
- `Outcome::StepNotStarted` (`outcome.rs:421`) is built only for `.audit()`
  (`crates/orchestrator/src/audit.rs:215`, `crates/orchestrator/src/workbench/setup.rs:732`).

Problem: none of them is "what one completed browser action did", which is the enum's doc. Yet
every exhaustive match must carry them: `summary`, `next_steps` and `measurements` in
`outcome.rs`, `retained` in `audit.rs`, and `self_check/hints.rs`. The variant
`Outcome::CompositionProgress` also has the same name as the struct
`composition::CompositionProgress`, which `Outcome::CompositionRan` holds.

Proposed change: move these sentences into `crates/orchestrator/src/language/composition.rs`, for
example as a `CompositionProgress::live_summary`, and drop the variants.

Ratings: payoff med, risk low, effort S. Model text: none.

Verdict: partial. The three variants, their builders and the name clash are as cited.
Corrections: `next_steps` names none of them, it ends in `_ => vec![]` (`outcome.rs:1060`); and
`workbench/setup.rs:732` sits in that file's tests (`setup.rs:708-709`), so production builds
`StepNotStarted` only in `AuditRecord::not_started` (`audit.rs:215`). Dropping that variant is
not low risk: only outcome and refusal constructors may author an `AuditProjection`
(`language/audit.rs:13-16`), and its sentence is the persisted summary of the not-started line
(`tests/golden/audit/not-started.json:13`). Move the two live-history sentences, and keep a typed
author for the not-started line.

Class: refactor.

arch-1: none.

## LANG-9: Phrasing helpers are reimplemented across modules

Kind: duplication.

Where:

- `counted`, `counted_or_none`, `article` and `one_line_bounded` live inside `outcome.rs` as
  `pub(super)` helpers (`outcome.rs:2485` to `2556`). `counted` groups digits (`grouped`,
  `outcome.rs:2558`).
- Plurals are written by hand in `composition.rs:99` to `104`,
  `crates/orchestrator/src/language/coverage.rs:99`, `audit.rs:357` to `370`,
  `audit_health.rs:103`, `crates/orchestrator/src/language/deployment.rs:1077` to `1093` and
  `crates/orchestrator/src/language/workbench.rs:697` (`tabs_count`, which is `counted_or_none`).
- Lists are joined three ways: `outcome.rs:49` (`browser_choices`), `deployment.rs:574`
  (`either_of`, "a, b, or c") and `crates/orchestrator/src/language/setup.rs:1129` (`join_list`,
  "A, B and C").
- "no value" is defined twice: `crates/orchestrator/src/language/field_echo.rs:31` and
  `field_problem.rs:44`.

Problem: general phrasing lives inside the 4,784-line `outcome.rs`, so sibling modules write their
own. The copies already differ: no digit grouping in the hand-written plurals, and two
serial-comma styles.

Proposed change: one small `phrase.rs`, used by every module.

Ratings: payoff med, risk low, effort S-M. Model text: none if the helpers only move. Adopting
`counted` everywhere would add digit grouping (1,000 and up) and one serial-comma style, which
changes words people see.

Verdict: partial. The joiners, the two "no value" constants and the hand plurals are as cited.
Corrections: `counted_or_none` is private (`outcome.rs:2550`); only `one_line_bounded`, `article`
and `counted` are `pub(super)` (`outcome.rs:2485`, `2513`, `2520`). And
`language/audit.rs:357-370` already calls `counted`; it hand-writes only "its value" or "their
values", which `counted` cannot say. New: `audit_health.rs:112-116` has no singular, so one
skipped entry reads "1 unreadable history entries were skipped."

Class: refactor.

arch-1: none.

## LANG-10: mod.rs is a 2.6k-line catch-all

Kind: structure and naming.

Where:

- `mod.rs` is the module root, about 830 lines of request types (`mod.rs:119` to `949`) and about
  1,600 lines of decoding and validation (`mod.rs:975` to `2579`).
- `catalog.rs` is mounted as `#[path = "catalog.rs"] mod tool_catalog;` (`mod.rs:30`), beside
  `self_check/catalog.rs`. The alias leaks: `self_check/catalog.rs:14` imports
  `crate::language::tool_catalog::EXAMPLE_UPLOAD_PATH`.
- The root doc (`mod.rs:1`) names "the complete model-facing catalog", which `mod.rs` does not
  hold, and none of the person-facing modules (`setup`, `deployment`, `workbench`, `policy` and
  others).
- `language::history` holds retained activity records. Its name clashes with browser history
  (`HistoryRequest` at `mod.rs:273`, `NavigateHistory` at `mod.rs:388`, `HISTORY_FORWARD` at
  `mod.rs:1367`).
- `history.rs:271` also holds `COMPOSITION_STEP_LIMIT`, which bounds the flow decoder
  (`mod.rs:1732`), the catalog (`catalog.rs:1036`) and audit tools (`audit.rs:76`).

Problem: the decoder's file mixes the root, the types and the validation, while its twin hides
behind an alias. Names point readers to the wrong place.

Proposed change: split into `operation.rs` and `decode.rs`. Use a plain `mod catalog;`; modules
and functions live in different namespaces, so it can still re-export `catalog::catalog`. Rewrite
the root doc so it names both the model-facing and the person-facing modules. Move
`COMPOSITION_STEP_LIMIT` into `composition.rs`.

Ratings: payoff med, risk low (pure moves), effort M. Model text: none.

Verdict: confirmed. The ranges hold (831 and 1,605 lines); 2.6k counts the 2,580 lines before
the tests, of 3,177. A plain `mod catalog;` beside `pub use catalog::catalog` is legal Rust,
because a module and a function live in different namespaces, and `self_check` still reaches the
private module as a descendant. `COMPOSITION_STEP_LIMIT` is also read at `workbench/history.rs:5`
and `161`, a surface-lane file the move must edit.

Class: refactor.

arch-1: none.

## LANG-11: Docs that no longer match the code

Kind: docs.

Where:

- `Outcome::next_steps` (`outcome.rs:999`) and `Refusal::next_steps` (`outcome.rs:1805`) say
  "zero or one" steps. They return two: `outcome.rs:1046` to `1050`, `1895` to `1898` and `1954`
  to `1959`.
- `setup.rs:10` says the setup view has "four stages", then lists five, which match the five
  `Stage` variants (`setup.rs:214`).
- `outcome.rs:414` and `audit.rs:65` still say "flow or sequence", after `browser_sequence` was
  removed.

Problem: a reader who trusts these comments misreads the result contract and the setup journey.

Proposed change: correct the three comments.

Ratings: payoff low-med, risk none, effort S. Model text: none.

Verdict: confirmed. All three comments are as cited (`outcome.rs:999` and `1805`; `setup.rs:10`
against the five variants at `setup.rs:214`; `outcome.rs:414` and `language/audit.rs:65`), and
the two-step returns are at `outcome.rs:1046-1050`, `1895-1898` and `1954-1959`.

Class: refactor.

arch-1: none.

## LANG-12: Small cleanups

Kind: concern, dead layers.

Where:

- `crates/orchestrator/src/language/environment.rs:160` to `210` reads the machine: environment
  variables and `/proc/sys/kernel/osrelease`. It is the only language module with I/O. Every other
  module is pure words.
- `object()` (`catalog.rs:1151`) only forwards its arguments to `raw_object()`.
- `invites_control()` (`crates/orchestrator/src/language/readiness.rs:117`) always returns true.
  It still travels to the window snapshot (`crates/orchestrator/src/workbench/mod.rs:1751`), to
  doctor (`crates/orchestrator/src/cli/doctor.rs:640`) and to the page
  (`crates/orchestrator/ui/lib/view.js:911`).
- `page_recovered_summary` and `tab_already_closed_summary` (`outcome.rs:2278`, `2287`) are `pub`,
  but only `Outcome::summary` calls them.

Problem: machine detection sits in the words module, and three small layers earn nothing.

Proposed change: move `EnvironmentFacts::observe`, `resolve` and `current` to the platform side
and keep the phrase table. Inline `object()`. Make the two summaries private. Drop
`invites_control` only if the window's contract allows it.

Ratings: payoff low, risk low, effort S. Model text: none. Dropping `invites_control` changes the
window's data contract (`ui/lib/view.js:911`), a person-facing surface.

Verdict: partial. `object()`, `invites_control()` and the two `pub` summaries are as cited.
Corrections: only `EnvironmentFacts::observe` (`environment.rs:161-168`) reads the machine, and
`current()` (`213-215`) through it; `is_wsl` and `resolve` (`170-210`) are pure.
`cli/doctor.rs:640` is a test. Production doctor carries the flag in `doctor --json`, which
serializes the whole `ReadinessSummary` (`cli/doctor.rs:298`, `workbench/mod.rs:1738`), so
dropping it changes that JSON too. Since S1 the wheel takes its act from `runtime.offers` and
keeps `invites_control` only as a guard (`ui/lib/view.js:910-911`).

Class: refactor.

arch-1: none. S1 (landed) moved the wheel onto `offers` and kept the guard.

## Overall

What is healthy: every module opens with a role doc. The newer vocabularies (`area`,
`page_refusal`, `field_problem`, `readiness`, the environment table) are closed enums with their
words beside them. The self_check schema and hint checkers are strong guards. The weakness is that
the decoder, the catalog, the capability map, the settlement list and `work/` each restate the
same per-tool facts, and many tests exist only to keep those copies in step. The single change that
would help most is a typed, table-driven decoded language: one `Tool` table, enums for the closed
fields and one block of bounds, read by the catalog, the decoder and the executor alike (LANG-1,
LANG-3 and LANG-5 together, with LANG-3's wait part landing through C17).

## Skeptic summary

Verdicts: 7 confirmed (LANG-1, 2, 4, 6, 7, 10 and 11), 5 partial (LANG-3, 5, 8, 9 and 12), 0
refuted. No finding's core fails. The partials cite a test as production, misread a reachable arm
or a helper's visibility, or carry a proposal step that is not a refactor (LANG-3's scroll
default, LANG-8's `StepNotStarted` drop).

Tags follow each proposal as written. All twelve are refactors. Model text moves only in LANG-3,
through C17's duration sentence or a decode-time scroll default, and in LANG-6, by one character
at the bound. Done as proposed, none changes persisted bytes, the frozen extension package or the
locked relay and edge source.

Small defects met while checking, outside the findings' claims:

- A wheel scroll that omits `direction` is refused (`mod.rs:1989-1992`), though the schema
  advertises "down" for it (`catalog.rs:656-660`).
- A wheel scroll accepts any `amount` and ignores it (`mod.rs:1988-2016`).
- A saved result's summary can pass `SUMMARY_MAX_CHARS` once coverage qualifies it
  (`coverage.rs:79-86`, `work/receipt.rs:35-37`).
- One skipped history entry reads "1 unreadable history entries were skipped."
  (`audit_health.rs:112-116`).
- The page-text handle test cannot match a real `diag_` cursor or `invocation_` id
  (`field_problem.rs:888-899`).

The three to do first:

1. LANG-4. A requirement set is an authorization fact, and today it has two homes and two test
   tables. The change is small, compile-checked and changes no word, as long as the lookup is
   total and fails closed.
2. LANG-1, starting with a test that checks every advertised default against the value the
   decoder applies, as `mod.rs:2625` does for `timeout_ms`. That closes the untested drift at
   once; the bounds block can follow, with the catalog golden guarding it byte for byte.
3. LANG-6. It is small, and it is the one finding about the summary contract: name the suffixes,
   derive the bounds, and decide in the same commit whether the coverage qualifier is bounded.

LANG-11 can ride along with any of them; it only corrects comments. LANG-5 comes after LANG-1 and
LANG-4, because its `Tool` table would absorb parts of both. LANG-3's wait part waits for C17, and
its declined part needs an ADR-0203 amendment first.
