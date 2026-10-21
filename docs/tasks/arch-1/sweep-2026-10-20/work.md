# Second sweep, 2026-10-20: work, workspace and governance

This sweep read `crates/orchestrator/src/work/` (execution of model-requested work),
`crates/orchestrator/src/workspace/` (the workspace aggregate) and
`crates/orchestrator/src/governance/` (policy, managed policy and the audit-availability rule). It
was read-only, on `dev` at `8f6a43ec`, and looked for magic literals, naming, structure and module
docs against the "Code style" and "Product and architecture invariants" sections of AGENTS.md.
Nothing was edited, built or run. Two constraints bound every change. Audit event names and policy
words are persisted, so renaming one is a migration, and each section says when it touches one. The
result goldens (`tests/golden/model/*.json`) pin model-visible outcomes, so "golden-neutral" below
means nothing the model sees changes. Paths are relative to `crates/orchestrator/src/` unless they
start with `docs/` or `tests/`. Ratings are payoff and risk (high, med or low) and effort (S, M or
L), and the findings are ranked by payoff over risk. Several findings restate parts of the
2026-10-19 sweep (`docs/tasks/arch-1/findings.md`) that ADR-0203 already decided in slices still
`todo` on `dev` (C12b, C17 and C19c). Each such section says so under "Prior record", and where
ADR-0203 lists a proposal made here under "Considered and not taken", adopting it needs an
amendment.

## WORK-1: The upload ceiling is written four times

Kind: magic literal.

Where:

- `language/mod.rs:2142` (`UPLOAD_AGGREGATE_BYTES`).
- `work/forms.rs:1381` (`decode_inline_files`, a function-local `UPLOAD_AGGREGATE_BYTES`).
- `work/mod.rs:3005` (`load_physical_files`, as `MAX_UPLOAD_BYTES: u64`).
- `workspace/mod.rs:2274` (`register_image`, a function-local `UPLOAD_AGGREGATE_BYTES`).
- An unexplained `7_000_000` cap at `work/reading.rs:399` (`screenshot`).

Problem: one limit has two names, two types and three function-local copies, which can drift
apart.

Proposed change: make the language constant `pub`, use it everywhere, and derive the base64 cap
from it.

Ratings: payoff high, risk low, effort S. Golden-neutral. No persisted name.

Prior record: ADR-0203 Decisions 5.2 and 6.4 (slice C12b, `todo`) make the inline-file ceiling one
constant, checked once at intake, and make the `7_000_000` check read the budget constant. The
path-upload copy (`work/mod.rs:3005`) and the captured-image copy (`workspace/mod.rs:2274`) are not
named there; C12b can take them. LANG-1 in `language.md` covers the decoder's own bounds.

Verdict: partial. The four 5,000,000 copies, two names and two types are as stated. The base64
cap must not be derived from the upload ceiling: `7_000_000` (`work/reading.rs:399`) bounds a
screenshot receipt, whose budget is the adapter's `MAX_BASE64_CHARS`, 6,000,000
(`browser/adapter/chromium/protocol4/geometry.rs:42`), the constant Decision 5.2 means. 5,000,000
bytes as base64 is 6,666,668 characters, a third number. The copies also count differently:
intake estimates `data.len() / 4 * 3`, padding included (`language/mod.rs:2236`), so a padded
inline set of exactly 5,000,000 bytes is refused, against `docs/1.0/LANGUAGE.md:963`, while a
5,000,000-byte path passes (`work/mod.rs:3015`). The exact count already exists as
`base64_decoded_len` (`work/forms.rs:1404`).

Class: refactor, defect (the refusal at exactly 5,000,000 bytes; it needs one test).

arch-1: C12b (Decisions 6.4 and 5.2). It does not name the path and captured-image copies.

## WORK-2: The authorize-or-refuse block is written 27 times

Kind: duplication.

Where:

- `json!({"reason": decision.reason.as_str()})` appears in `work/recording.rs` (7), `work/mod.rs`
  (6), `work/navigation.rs` (5), `work/pointer.rs` (4), `work/forms.rs` (3), `work/flow.rs` and
  `work/reading.rs`. 26 copies go to `blocked` or `blocked_at`, and one to `channel_preflight`
  (`work/mod.rs:1212`).
- `with_authorized_tab` (`work/mod.rs:736`) is `with_authorized_optional_target`
  (`work/mod.rs:767`) without a target.
- `resolve_semantic` (`work/mod.rs:1059`) repeats the select, authorize and refuse steps of both.

Problem: every denial path re-spells the same facts and the same early return, so a change to what
a denial carries has 27 places to miss.

Proposed change: let `blocked_at` fill in `reason` from the decision when the facts lack it, as it
already adds `denial_id` and `policy_rule` through `with_denial_facts`. Add
`refuse(context, decision, physical_id)`, and keep one `with_authorized_*`.

Ratings: payoff high, risk low, effort S. Golden-neutral. No persisted name.

Prior record: ADR-0203 Decision 5.1 (slice C12b, `todo`) has one `Terminal` constructor write each
refusal's model code, which removes these 27 literals. It does not cover the `with_authorized_*`
and `resolve_semantic` duplication. Slice C10b (landed) already made those helpers admit
`context.requirements`.

Verdict: confirmed. 27 copies, per file as stated: 26 go to `blocked` or `blocked_at` and one to
`channel_preflight` (`work/mod.rs:1212`). Seven more refusals send the same decision's reason under
another name or beside other keys (`work/forms.rs:59`, `work/mod.rs:729` and `2003`,
`work/navigation.rs:341`, `355`, `697` and `729`), so every caller of `blocked_at` already sends
it, and filling it in is byte-identical: serde_json is built without `preserve_order`, so keys
sort. A three-argument `refuse` fits 22 of the 26: three are `blocked_at` with a URL
(`work/navigation.rs:191`, `411` and `438`), and `work/mod.rs:1960` is not repeat-safe.

Class: refactor.

arch-1: C12b (Decision 5.1) for the literals; none for merging the `with_authorized_*` helpers and
`resolve_semantic`.

## WORK-3: 23 result envelopes are built by hand

Kind: structure (the one-typed-Outcome invariant).

Where: direct `InvocationResult::new` calls, each with eight positional arguments and a bare
repeat-safe bool:

- `work/mod.rs`, 17 calls, for example `execute_prepared` at 354, 407 and 456, `browser_failure`
  at 2010, 2038 and 2135, and `workspace_refusal` at 2248.
- `work/forms.rs`, 4 calls, at 304, 880, 1006 and 1102.
- `work/composition.rs:185`.
- `work/navigation.rs:701`.

Problem: each site pairs one `Refusal`'s or `Outcome`'s `summary`, `next_steps`, `observed` and
`audit` by hand. AGENTS.md's rule that one typed `Outcome` gives a completed action's sentence, its
next steps and its measurements therefore holds by habit, not by type.

Proposed change: add `Terminal::refused(&Refusal, shape, facts, decision, physical_id)` and
`Terminal::completed(&Outcome, ...)`, and make `InvocationResult::new` private to them.

Ratings: payoff high, risk low, effort M. Golden-neutral; the goldens verify it. No persisted name.

Prior record: ADR-0203 Decision 5.1 (slice C12b, `todo`) already calls for one `Terminal`
constructor that writes the model code. Decision 4.5 sends the hand-built already-closed terminal
(`work/navigation.rs:701`) through `succeeded`, so its next step reaches the agent. This finding
sizes that constructor at 23 sites and extends it to `Outcome`.

Verdict: partial. The 23 production calls are as listed (`work/composition.rs:255` is a test).
Four depart from one typed value on purpose: `work/mod.rs:362` sends `error.guidance()`, not
`Refusal::InvalidRequest`'s step (`language/outcome.rs:1810`), and
`tests/golden/model/refusal.json` pins it; `work/composition.rs:180` qualifies the summary by
coverage and `200` adds unconfirmed history to the audit; `work/mod.rs:2051` adds `for_gesture`;
and `work/navigation.rs:709` sends `vec![]` where `Outcome::TabAlreadyClosed` has a step
(`language/outcome.rs:1004`). So the constructors need overrides, and the already-closed site
changes what the agent sees, as Decision 4.5 intends. The five model goldens reach four of the 23
sites, so they do not verify the rest.

Class: refactor, model-visible (the already-closed step).

arch-1: C12b (Decisions 5.1 and 4.5); none for the `Outcome` constructor.

## WORK-4: The `reason` fact words are raw strings

Kind: vocabulary (model-visible).

Where:

- `browser_reason` (`work/mod.rs:3233`).
- `routing_refusal` (`work/mod.rs:2631`).
- `browser_failure` (`work/mod.rs:1946`).
- `unconfirmed_reason` (`work/mod.rs:2969`).
- `cancelled_before_start` (`work/mod.rs:2375`).
- `execute_prepared` (`work/mod.rs:361`, `414` and `463`).
- `work/reading.rs:404`.
- "deadline" and "browser_effect_unknown" appear five times each in `work/mod.rs`, and
  "cancelled", "document_unavailable" and "browser_local_interlock" three times each.

Problem: a vocabulary the model reads lives as literals at call sites, against AGENTS.md's rule
that such a vocabulary lives in one domain module. The language module already does this properly
for workspace errors (`WorkspaceReason::as_fact`, `language/outcome.rs:2124`).

Proposed change: add `Refusal::reason_fact()` next to its summary, and have the WORK-3 constructor
write it.

Ratings: payoff high, risk low, effort M. Model-visible: keep the strings unchanged. The model
goldens (`tests/golden/model/refusal.json`, `channel_refusal.json` and `stale_refusals.json`) pin
some of them. Not persisted: the audit stores `AuditRefusal`.

Prior record: first-sweep V-01. ADR-0203 Decision 5.1 (slice C12b, `todo`) decides exactly this as
`Refusal::reason()`, deletes `browser_reason`, `unconfirmed_reason` and every call-site literal, and
pins each (model code, audit tag) pair in a table test. C12b expects the result goldens to be
regenerated where a prose code becomes closed. The literals are still present on `dev`.

Verdict: confirmed. Every cited literal is there (the screenshot word is at `work/reading.rs:405`),
and the counts hold; one "browser_local_interlock" is `denial_presentation` reading the fact back
(`work/mod.rs:2323`), its only production reader. The audit keeps `AuditRefusal`
(`audit.rs:177`), so nothing persisted moves. One variant can carry several words today:
`Refusal::BrowserContract` takes whatever `browser_reason` returns (`work/mod.rs:2131` and
`2142`), so `reason()` needs the closed causes C12b names, not one word per variant.

Class: refactor, model-visible (C12b regenerates the result goldens where a code becomes closed).

arch-1: C12b (Decision 5.1).

## WORK-5: Closed input vocabularies are decoded as String

Kind: vocabulary.

Where: `Wait.condition`, `RunFlow.on_error`, `HandleDialog.action` and `Record.action`
(`language/mod.rs:834` to `910`) are matched again by literal string in:

- `work/mod.rs:2545` and `2621`;
- `work/recording.rs:46`, which ends in `unreachable!("recording action was validated")`;
- `work/forms.rs:801`, `899` and `1043`;
- `work/documents.rs:137`;
- `work/interaction.rs:48` and `95`;
- `work/flow.rs:80` and `121`.

Problem: a typo in any of these literals compiles, and a new value must be found in every module
that matches it again.

Proposed change: decode them to snake_case serde enums, match exhaustively, and use `as_str()` where
a fact or `BrowserCommand::Observe` needs the word.

Ratings: payoff high, risk low, effort M. Edits `language/`. The catalog golden stays the same if
the words do. No persisted name.

Prior record: first-sweep W-03 and V-08, and LANG-3 in `language.md`, which lists the same fields
and more. ADR-0203 Decision 8 (slice C17, `todo`) makes the wait conditions one closed
`language::WaitCondition`, decoded by `TryFrom` inside validation so the curated refusal words stay,
with the catalog golden byte-identical. ADR-0203 lists closed enums for `on_error`, `Record.action`,
`HandleDialog.action` and Diagnose's fields under "Considered and not taken" ("They fix no defect a
person or agent meets"), so that part of this change needs an ADR-0203 amendment. The evidence here
is the `unreachable!` arm and the repeated literal matches. If it is taken, the `TryFrom` route
keeps the curated refusal words, which plain serde would not.

Verdict: confirmed. All eleven matches are string compares (the `unreachable!` arm is
`work/recording.rs:78`), and validation matches `selector_present` once more
(`language/mod.rs:2299`). Plain serde enums would replace the curated words the model reads
(`validate_choice`, `language/mod.rs:2465`), so only the `TryFrom` route is safe. No defect today:
"payoff high" is too strong for the three fields ADR-0203 declined.

Class: refactor (model-visible if plain serde words replace the curated ones).

arch-1: C17 (Decision 8) for `Wait.condition`; the other three reopen "Closed enums for
`on_error`, `Record.action`, `HandleDialog.action` and Diagnose's fields".

## WORK-6: work/mod.rs does seven jobs

Kind: structure (oversized file, stale module doc).

Where: `work/mod.rs`, 3,515 lines:

- the invocation lifecycle;
- ten admission wrappers, `audit_decision` to `authorize_close_setting` (1184 to 1383);
- browser choice and recovery, `admitted_browser` to `await_or_recover_browser` (1490 to 1665),
  with the `RecoveryDecision` and `RecoveryWaitError` mapping duplicated at 1592 and 1651;
- a 200-line `browser_failure` (1946);
- upload file loading and a MIME table, `load_physical_files` and `media_type` (3004 to 3073);
- recording word tables (2565 to 2592);
- `Operation` projections (2538 to 2776, and more in `work/pending.rs:164` to `200`);
- 250 lines of test fixtures (3261 to 3515).

The module doc (line 1) names none of these, and it claims the completion path, which lives in
`work/receipt.rs`.

Problem: a reader looking for any one of these jobs pages through the other six, and the doc points
at the wrong file.

Proposed change: split the file into `admission.rs`, `routing.rs`, `failure.rs` and `upload.rs`,
move the fixtures to `work/tests/mod.rs`, put the handle accessors on `Operation`, and update the
module doc.

Ratings: payoff high, risk low, effort M. Golden-neutral. No persisted name.

Prior record: none in ADR-0203.

Verdict: partial. The size, the ranges and the jobs hold. Two details do not. The two recovery
matches share four of six arms: `Ready` and `Manual` map differently (`work/mod.rs:1596` and
`1609` against `1652` and `1658`). And the doc is not wrong to claim the completion path: an inner
doc in `mod.rs` documents module `work`, which contains `work::receipt`; it is the list of jobs
that is short.

Class: refactor.

arch-1: none.

## WORK-7: Dead code and an indirection that guards nothing

Kind: dead code.

Where:

- `CompletionGate` (`work/result.rs:193`), created per call at `work/mod.rs:346` and
  `work/receipt.rs:160`, and completed then taken at `work/receipt.rs:129` to `131`.
- `WorkspaceLease::tab_owner` and `WorkspaceLease::target_owner` (`workspace/mod.rs:2088` and
  `2095`).
- `substitute_references` (`work/flow.rs:161`) and its caller's branch at `work/flow.rs:51` to `53`.

Problem: the gate is completed and taken straight away, so it can never see a second completion;
passing `Terminal` by value already guarantees one. The two lease methods have no callers; the
executor calls the store's versions. `substitute_references` returns
`Result<Option<Value>, String>`, but no leaf returns `None`, so the "a result reference did not
resolve" branch never runs.

Proposed change: delete all three.

Ratings: payoff med, risk low, effort S. Golden-neutral. No persisted name.

Prior record: first-sweep W-11. ADR-0203 Decision 4.4 (slice C19c, `todo`) deletes
`CompletionGate`. The two lease methods and the `Option` layer are new.

Verdict: confirmed. The gate is made per call and completed then taken at once
(`work/receipt.rs:129` to `131`, `160`). Nothing in `crates/` calls the two lease methods.
`substitute_references` returns `Ok(Some(..))` on every path (`work/flow.rs:161` to `204`);
"delete" there means the `Option` layer, not the function. WORK-14's proposal would give the two
lease methods callers, so the findings conflict; WORK-14's verdict below favors deleting them.

Class: refactor.

arch-1: C19c (Decision 4.4) for `CompletionGate`; none for the lease methods and the `Option`
layer.

## WORK-8: Typed data round-trips through the model's JSON

Kind: convoluted control flow.

Where:

- `denial_presentation` (`work/mod.rs:2319`).
- `limited_coverage` (`work/composition.rs:209`) and the write it reads back, `work/receipt.rs:39`.
- The `COVERAGE` constant (`work/composition.rs:17`) beside the literal `"coverage"`
  (`work/receipt.rs:39` and `work/composition.rs:179`).
- The WorkBlocked test in `complete_terminal` (`work/receipt.rs:42` to `52`).

Problem: `denial_presentation` works out the presentation from `tool == "browser_tabs"` and the
`facts.reason` string. `limited_coverage` parses back the coverage that `work/receipt.rs:39` just
serialized from the frame's typed findings. The `COVERAGE` constant competes with the literal it
was meant to replace. The WorkBlocked test is a four-clause boolean over `ReasonCode` and
`AuditRefusal`.

Proposed change: carry `coverage` and the `DenialPresentation` on `Terminal`, set where the refusal
is built.

Ratings: payoff med, risk low, effort S. Golden-neutral. No persisted name.

Prior record: first-sweep W-01 and W-08. ADR-0203 Decision 5.6 (slice C12b, `todo`) chooses the
in-page notice and the desktop toast from the typed `Terminal` and deletes the tool-name compare in
`denial_presentation`. Decision 6.8 (slice C17, `todo`) has composition read a step's coverage from
`terminal.audit.coverage()`, with the `coverage` key one constant. ADR-0203 lists "A typed coverage
field on `Terminal`" under "Considered and not taken" ("The typed value already travels in the
audit projection"), so for coverage the ADR's route replaces this finding's field. The four-clause
WorkBlocked test belongs with Decision 5.6's change.

Verdict: confirmed. Each site is as described. The notice half is a defect a person sees: a flow
step whose tab close the policy refuses shows the generic guardrail notice, not the kept-tab one,
because `denial_presentation` (`work/mod.rs:2319`) sees `browser_flow`, not `browser_tabs`. The
coverage half needs no new field: `AuditProjection::coverage()` exists (`language/audit.rs:56`),
the route Decision 6.8 takes.

Class: refactor, defect (the flow's notice; C12b names its test).

arch-1: C12b (Decision 5.6) for the notice; C17 (Decision 6.8) for coverage. A coverage field on
`Terminal` reopens "A typed coverage field on `Terminal` (W-08)".

## WORK-9: The same name with a different contract

Kind: naming.

Where:

- The executor's `authorize_landing` and `authorize_tab_close` (`work/mod.rs:1314` and `1351`), and
  `AuthoritySnapshot::authorize_landing` and `AuthoritySnapshot::authorize_tab_close`
  (`governance/mod.rs:682` and `650`).
- `admit_tab` in `work/pending.rs:103` and in `workspace/mod.rs:1036`.
- `WorkspaceLease::confirm_tab_closed` (`workspace/mod.rs:2326`).
- `Decision::allow` and `Decision::permitted`, `Decision::deny` and `Decision::refused`
  (`governance/mod.rs:321` to `346`).

Problem: the executor's two methods share names with snapshot methods that mean something
different. The executor's versions record the frame's evidence, and its tab close also checks
Action on the tab's host and the runtime controls. `admit_tab` is a crossing check in work but a
binding check in the workspace. `confirm_tab_closed` actually forgets a tab, while "confirm"
elsewhere means ADR-0203's binding confirmation. `Decision` has two names for each constructor.

Proposed change: rename to `check_landing`, `check_tab_close`, `admit_crossing` and
`forget_closed_tab`, and keep one constructor for each.

Ratings: payoff med, risk low, effort S. Not persisted. Golden-neutral.

Prior record: none in ADR-0203.

Verdict: confirmed. Names and lines hold; `Decision::allow` and `deny` are private, and
`permitted` and `refused` are their public wrappers (`governance/mod.rs:320` to `346`). The
workspace `admit_tab` is private to `AggregateState` (`workspace/mod.rs:1036`), so the clash costs
readers, not callers. ADR-0203 names `confirm_tab_closed` (C2) and "the frame's
`authorize_landing`" (Decision 7.7); a rename leaves those as history.

Class: refactor.

arch-1: none.

## WORK-10: Timings and repeated limits without names

Kind: magic literals.

Where, durations:

- 500 ms at `work/mod.rs:314` (`show_waiting`).
- 2 s twice: `work/mod.rs:989` (`finish_with_expectation`) and `work/mod.rs:1677`
  (`compensate_close`).
- A 100 ms poll at `work/forms.rs:954`, right beside the named 20 ms `DURATION_POLL`.
- 50 ms at `workspace/mod.rs:1524` (`acquire_until`).
- A backoff of 15 s, 2^8 and 10% at `governance/managed/mod.rs:544` to `548` (`schedule`).

Where, limits:

- 8192 is both `URL_LIMIT` (`work/documents.rs:10`) and `TAB_URL_CHARS` (`workspace/mod.rs:54`).
- The title length 500 is a literal at `work/reading.rs:72`, beside `TAB_TITLE_CHARS`.
- 255, 100 and 160 are literals at `work/mod.rs:3045`, `work/forms.rs:1393` and `1394`,
  `work/documents.rs:489` and `work/recording.rs:559`.
- The next-step cap of 2 is written three times: `work/result.rs:154`, `language/outcome.rs:1049`,
  and the private `NEXT_STEPS_MAX` in `language/field_problem.rs:48`.
- `bounded()` is defined twice, at `work/mod.rs:3229` and `workspace/mod.rs:2678`.

Problem: AGENTS.md asks for a repeated literal to become a named constant beside its use. These
values either repeat across modules or hide a choice, such as a poll rate or a budget, in a bare
number.

Proposed change: name each value next to where it is used, and share the ones used across modules.

Ratings: payoff med, risk low, effort S. Golden-neutral. No persisted name.

Prior record: none in ADR-0203. LANG-6 in `language.md` covers the next-step cap from the language
side.

Verdict: confirmed. Every cited line holds. Share by fact, not by value: the two 2 s are an
expectation's observe budget (`work/mod.rs:989`) and a compensating close (`1677`); 160 bounds a
document id (`work/documents.rs:489`) and an export reason's text (`work/recording.rs:559`); and
`URL_LIMIT` counts bytes (`work/documents.rs:229` and `490`) where `TAB_URL_CHARS` counts
characters (`workspace/mod.rs:79`). 255 and 100 are also the decoder's bounds
(`language/mod.rs:2221` and `2222`, LANG-1).

Class: refactor.

arch-1: none.

## WORK-11: Governance says one thing three ways

Kind: naming, with persisted words.

Where:

- One concept, four names: the policy key `content.security.sacred_domains`
  (`PolicySetting::SacredDomains`, `governance/manifest.rs:189`), `ReasonCode::ProtectedHost`
  (`governance/mod.rs:224`), `PolicyRule::SacredHost` (`governance/mod.rs:281`), and the
  `never_touch` field and functions with `sacred_hosts()` (`governance/mod.rs:458`, `579` to
  `614`, and `NeverTouched` and `Untouched` at `1008` to `1035`).
- Three enums for the policy layers: `AuthorityTier` (`governance/mod.rs:439`), `LayerKind`
  (`governance/effective.rs:45`) and `PolicyAuthor` (`language/outcome.rs:291`), bridged by
  `layer_kind()` (`governance/effective.rs:569`) and a match in `channel_refusal`
  (`governance/mod.rs:539` to `542`). The tier word "managed" is the constant `MANAGED_TIER`, but
  "user" is a literal (`governance/mod.rs:448`).
- `governance::audit`, the availability rule, sits beside `crate::audit` (the writer),
  `language::audit` and `language::audit_health`.
- `governance/mod.rs:7` re-exports `crate::audit::{AuditSink, JsonlAuditSink}`, so
  `JsonlAuditSink` is imported by two paths (`service/mod.rs:66` and `2502`).
- The module doc at `governance/mod.rs:3` promises "minimized audit intent", which no longer
  exists.

Problem: one concept carries four names, the policy layers carry three enums mapped by hand, the
audit-availability rule shares a name with the audit writer, and the module doc describes code that
is gone.

Proposed change: use one internal word for never-touch, fold `AuthorityTier` into `LayerKind` with
a `tier_word()`, rename `governance::audit` to `audit_requirement`, drop the re-export and fix the
doc.

Ratings: payoff med, risk low, effort S. Persisted: the words `sacred_domains`, `sacred_host`,
`protected_host` and the tier words `managed` and `user` stay as they are. Renaming any of them is
a separate migration. Golden-neutral.

Prior record: first-sweep G-09. ADR-0203 lists "One tier vocabulary (G-09) ... in this batch" under
"Considered and not taken" as a refactor for whoever next touches that code, so the tier part fits
after the batch. The policy lane is closed (`docs/tasks/arch-1/LEDGER-policy.md`). The
`sacred_host` rule word came with Decision 7.2 (slice P2).

Verdict: partial. The lines hold; two parts need correcting. `ProtectedHost` is not another name
for never-touch: it also refuses every non-HTTP(S) scheme, crediting no layer
(`governance/mod.rs:697` and `698`), and the language adds a fifth name,
`BlockedReason::ProtectedHost` (`language/outcome.rs:270`). `LayerKind` has a third variant,
`Ghostlight` (`governance/effective.rs:51`), that no layer can be, so folding `AuthorityTier` into
it loosens `PolicyLayer.tier`; `PolicyAuthor` (`language/outcome.rs:291`) has the same two cases
and is the fold. The second `JsonlAuditSink` path is test code (`service/mod.rs:2502` and
`3182`).

Class: refactor, persisted (`managed`, `user` and the three never-touch words stay
byte-identical).

arch-1: the tier fold reopens "One tier vocabulary (G-09) and one table of home file names (V-09)
in this batch"; none for the rest.

## WORK-12: workspace/mod.rs keeps its tests and every registry in one file

Kind: structure.

Where:

- `workspace/mod.rs` has 4,820 lines, of which 2,073 are inline tests (from line 2747);
  `governance/` keeps its tests in `governance/tests.rs`.
- Lease methods repeat the lock-and-lookup preamble (lock the store, then look up
  `self.workspace`, mostly ending in `.ok_or(WorkspaceError::UnknownWorkspace)?`) 23 times, and the
  generation check 10 times.
- The scan for another workspace that owns a handle appears 7 times.
- `resolve_view_point` and `resolve_view_region` (`workspace/mod.rs:2486` and `2547`) share about
  35 lines.
- `TargetBook` (`workspace/mod.rs:563` to `731`) and the released-tab pool (`PooledTab` and its
  `AggregateState` methods, `923` to `1011`, and `settle_released` and `claim_released`, `1370` to
  `1416`) sit in the same file.

Problem: the aggregate's rules are hard to find among the tests and the repeated preambles, and a
change to view resolution must be made twice.

Proposed change: move the tests to `workspace/tests.rs`, `TargetBook` to `targets.rs` and the
released-tab pool to `released.rs`. Add the helpers `with_state`, `current(&SelectedTab)`,
`owned_elsewhere` and `resolve_view`.

Ratings: payoff med, risk low, effort M. Golden-neutral. No persisted name.

Prior record: none in ADR-0203. Slice C2 (landed) made `forget_tab` the one end of a binding, which
these helpers keep.

Verdict: confirmed. The line counts and the 23 lease locks hold (22 look up `self.workspace`, 17
end in `ok_or(UnknownWorkspace)?`). The tab generation compare is 9 in the lease
(`workspace/mod.rs:2218` to `2575`), not 10. Of the 7 owner scans, `owner_name`
(`workspace/mod.rs:1657`) is already shared and `controlled_elsewhere` (`1018`) keys on a physical
id, so the new helper takes 5 (`2069`, `2357`, `2412`, `2501` and `2564`). The view resolvers
share 30 lines plus the `SelectedView` build (`2493` to `2522` and `2556` to `2585`).

Class: refactor.

arch-1: none.

## WORK-13: perform_wait is about 340 lines with three copies of its ending

Kind: long function.

Where: `perform_wait` (`work/forms.rs:789` to `1125`).

Problem: its three branches (the duration wait at 801, `selector_present` at 899 and the adapter
observe at 1022) each repeat the settle step, the elapsed time, the status, `Outcome::Waited`, the
facts and the envelope. They contain pointless aliases such as `final_satisfied = satisfied` (983
and 1073). The selector branch leaves out the `readiness` fact the other two send.

Proposed change: have each branch measure `(satisfied, elapsed_ms, readiness)`, and let one
`finish_wait` build the rest.

Ratings: payoff med, risk med, effort M. Model-visible: the wait facts reach the model, so decide
the `readiness` gap deliberately. No persisted name.

Prior record: ADR-0203 Decision 8 (slice C17, `todo`) removes the condition literals here and gives
`WaitCondition` a method for the default settle, which replaces the `should_settle` logic at
`work/forms.rs:1042` to `1050`. Decision 5.1 (slice C12b, `todo`) deletes the unreachable let-else
in the selector branch (`work/forms.rs:900` to `908`). Slice C9 (landed) fixed the duration
branch's deadline. Do this with C17 or after it.

Verdict: confirmed. 337 lines; the three endings, the two `final_satisfied` aliases and the
missing `readiness` fact (`work/forms.rs:997` to `1004`) are as stated. The let-else at `900` to
`908` is unreachable, since `validate_wait` requires the selector (`language/mod.rs:2299` to
`2303`). No result golden holds a wait, so a `readiness` change needs its own test.

Class: refactor, model-visible (if the selector branch gains `readiness`).

arch-1: C17 (Decision 8) and C12b (Decision 5.1) take parts; none for `finish_wait`.

## WORK-14: Two doors into a leased workspace

Kind: invariant (one workspace aggregate, mutated under its lease).

Where:

- During a leased call the executor mutates through `WorkspaceStore`: `register_recovered`
  (`work/mod.rs:883`), `mark_target_removed` (`work/mod.rs:1890`) and `confirm`
  (`work/pending.rs:146`).
- It reads through the store too: `tab_owner` and `target_owner` (`work/mod.rs:2182` and `2185`),
  `tabs` (`work/mod.rs:2286`), `dialog_open` (`work/mod.rs:1722`), `stale_target_context`
  (`work/mod.rs:861`) and `browser_of` (`work/mod.rs:1520` and `1680`, `work/interaction.rs:26`).
- Store methods take `&WorkspaceId` (`tabs`, `opener`, `dialog_open`, `confirm`) or `&str`
  (`contains`, `browser_of`, `physical_tabs`, `tab_bindings`, `binding_state`).

Problem: the lease is documented as exclusive mutation ownership for one unit of work, but the
failure and success helpers (`browser_failure`, `workspace_refusal`, `succeeded`) never receive it,
so they reach around it.

Proposed change: carry `Option<&WorkspaceLease>` in `InvocationContext`, move these calls to the
lease, keep the store methods for asynchronous browser events, and use one id type.

Ratings: payoff med, risk med, effort M. Golden-neutral. No persisted name.

Prior record: none in ADR-0203.

Verdict: partial; harmless, neither an invariant breach nor a race. The calls are there, and the
list misses two writes, `apply_browser_dialog` through `note_dialog` (`work/interaction.rs:26` to
`28`) and `claim_released` on the shared pool (`work/navigation.rs:268` and `278`), and two reads,
`binding_state` (`work/pending.rs:123`) and `opener` (`work/navigation.rs:195` and `426`). The
lease is a flag beside one mutex (`workspace/mod.rs:1477` to `1491`), and by design browser events
write leased workspaces through the store: a commit moves a leased tab's generation
(`workspace/mod.rs:1799`, from `service/mod.rs:2405`). Every executor store call is made inside
the call that holds the lease, for its own workspace or the shared pool; the lease-free recording
calls name no tab and no target, so they make none of these writes. The store's `confirm` says
only the lease holder calls it (`workspace/mod.rs:1759`). The one gap is a generation, not a
lease: without `same_document`, `register_recovered` registers at the tab's current generation
(`workspace/mod.rs:1633`), so a commit between the recovery query and the registration offers a
candidate that the adapter then refuses as navigated
(`browser/adapter/chromium/protocol4/translate/observe.rs:92`). Moving `tab_owner` and
`target_owner` to the lease conflicts with WORK-7.

Class: refactor.

arch-1: none in ADR-0203. Slice C4 (landed) put `confirm` on the store on purpose, because "a
crossing holds no lease" (`docs/tasks/arch-1/LEDGER-core.md:145`).

## WORK-15: The host-pattern grammar is parsed five times

Kind: duplication (governance).

Where:

- Five readers of the `*`, `*.suffix` and exact patterns: `valid_host_pattern`
  (`governance/manifest.rs:688`), `pattern_specificity` (`governance/mod.rs:1215`, which lowercases
  on every call and does not trim), `host_matches` (`governance/mod.rs:2247`, which trims and
  lowercases), `ProbeSet::of` (`governance/mod.rs:1056`, which trims and lowercases) and
  `pattern_covers` (`governance/effective.rs:659`, which compares without case).
- Getting a URL's lowercase host is repeated five times: `governance/mod.rs:588` and `706`,
  `governance/evidence.rs:206`, `work/mod.rs:3167` (`observed_host`) and `workspace/mod.rs:451`
  (`TabHold::host`).
- `Decision::policy(PolicyAttribution { .. })` with `denial_bytes` is spelled out five times
  (`governance/mod.rs:501`, `530`, `601`, `662` and `951`).

Problem: each reader parses the grammar again with its own normalization, so a change to the
grammar, or a pattern that only one reader normalizes, can make validation, matching, probing and
the Policy page disagree.

Proposed change: parse a `HostPattern` once at load, and add one `url_host()` and one
`layer_denial()`.

Ratings: payoff med, risk med, effort M. Golden-neutral; `governance/tests.rs` covers the behavior.
No persisted name.

Prior record: ADR-0203 Decision 7.1 (slice P1, landed) made one reach evaluator over one probe set;
it does not cover the pattern parse. The policy lane is closed.

Verdict: partial. The five readers and their normalization are as stated, but they cannot
disagree today: `validate_patterns` runs `valid_host_pattern` on every grant and never-touch
pattern (`governance/manifest.rs:628` to `645`), which refuses whitespace, and every matcher
lowercases or ignores case. That is a latent risk, not a defect. `Decision::policy` is written six
times, not five (`governance/audit.rs:100` too), and the sites differ: `governance/mod.rs:951`
passes a grant id and `661` falls back to the snapshot id. Those bytes become the persisted
`denial_id`, so `layer_denial()` must keep them byte for byte. Only `governance/effective.rs:1078`
tests a mixed-case match, so the parse needs its own test. A sixth host read is `reusable_site`
(`workspace/mod.rs:1160`).

Class: refactor, persisted (`denial_id` stays byte-identical).

arch-1: none. P1 (landed) did not touch the parse.

## Overall

The area is well documented and clear about its invariants, but it keeps them by convention at
hundreds of call sites rather than through types, and that is where both the bulk and the drift
come from. The single change that would help most is WORK-3 combined with WORK-4, which also
absorbs WORK-2: let the typed `Refusal` or `Outcome` build the whole `Terminal`, including the
summary, next steps, audit, observation and `reason` word. That deletes several hundred lines and
makes "one completion path" something the compiler enforces. ADR-0203 Decision 5.1 already decides
the refusal half as slice C12b, still `todo` on `dev`. Landing C12b first, then extending its
constructor to `Outcome` and to the authorize-or-refuse block, avoids doing the work twice. WORK-5
and WORK-8 each restate a proposal ADR-0203 considered and did not take, so they need an amendment
or should follow the ADR's route. WORK-11's tier part is the refactor ADR-0203 left for whoever
next touches that code.

## Skeptic summary

Each finding was checked against the live tree on `dev` at `8f6a43ec`. Nothing was built or run.

- Confirmed: 9 (WORK-2, WORK-4, WORK-5, WORK-7, WORK-8, WORK-9, WORK-10, WORK-12 and WORK-13).
- Partial: 6 (WORK-1, WORK-3, WORK-6, WORK-11, WORK-14 and WORK-15).
- Refuted: 0.

WORK-14 is harmless. Its store calls are real, but the lease never guarded the data, and every
call it lists runs inside the call that holds the lease. WORK-15's disagreement is latent, because
validation refuses the patterns that would expose it. WORK-5 and the coverage half of WORK-8 would
reopen ADR-0203 entries, and WORK-7 and WORK-14 pull the two lease methods opposite ways.

Do first:

1. WORK-8's notice half (Decision 5.6, in C12b). It is the one defect here that a person meets: a
   flow step whose tab close the policy refuses shows the generic notice, not the kept-tab one.
   The change is small, and C12b already names its test.
2. WORK-1. One constant and an exact count, inside C12b's intake check. It fixes the one defect
   here that an agent can meet, the refused inline set of exactly 5,000,000 bytes, and it points
   the screenshot cap at the adapter's budget instead of a third number.
3. WORK-4 with WORK-2's literals, the core of C12b. It is already decided, it removes most of the
   drift these findings describe, and its table test and the result goldens catch a slip. All
   three fit in C12b's two commits, and WORK-7's deletions can follow.
