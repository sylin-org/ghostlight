# arch-1: policy ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Every slice landed: P10, P1, P2, P3, P4, P9, P5, P6, P7 and P8. The lane is closed. S4a is
unblocked, P5 unblocks C10c and S3, and P7 and P8 unblock S4b.

The second sweep (2026-10-20) opened this lane again on test-03. Its rows have all landed
(WORK-15, WORK-11 and P-lock; parked 2026-10-21). WORK-10's backoff names in
`governance/managed/mod.rs` ride with core-work's WORK-10.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| P10 | feat(governance): the runtime state names the acts it accepts | candidate | ux-2 closes | - | landed (570528ad, 4476fda8) |
| P1 | fix(governance): one evaluator says whether a capability can be used anywhere | candidate | ux-2 closes | see Notes | landed (3373e274, e45f09ed) |
| P2 | fix(governance): a never-touch refusal names the layer that wrote it | candidate | ux-2 closes | flatten already gone, see Notes | landed (bc00a84b) |
| P3 | fix(governance): one replay of recorded work | candidate | ux-2 closes | see Notes | landed (9c705f86) |
| P4 | fix(governance): one organization identity per publication | candidate | ux-2 closes | see Notes | landed (c2c958ff) |
| P9 | fix(governance): an owned user policy moved at load stays owner-only | candidate | ux-2 closes | see Notes | landed (994a6ffa) |
| P5 | refactor(governance): one publication is read whole | none | ux-2 closes | see Notes | landed (c5a37984) |
| P6 | refactor(governance): the unsigned managed file path goes | none | ux-2 closes | cross-lane core and surface call sites, see Notes | landed (67cf4811) |
| P7 | refactor(governance): one settings vocabulary | none | ux-2 closes | cross-lane core tests, see Notes | landed (1e12f49c) |
| P8 | refactor(language): the policy sentences live in language/policy.rs | none | ux-2 closes | cross-lane `language/mod.rs`, see Notes | landed (this commit) |

## Notes

- P10: `offers` names only the four acts. `ToggleHold` is a request, not an act: `apply_intent`
  turns it into Hold while active and Resume otherwise, then the table decides. One behavior
  changes on purpose: a Start session sent while paused no longer resumes, because a paused state
  does not offer it (the window's own table already said so). `apply_intent` now reads and changes
  the state under the observer lock, so a Resume that races an End session cannot undo it.
  `stop_is_terminal_and_idempotent` is renamed `stop_is_still_terminal` and also covers Hold and
  the toggle; `every_state_changes_only_through_an_act_it_offers` pins the whole table. S1 is
  unblocked.
- P1: `AuthoritySnapshot::reach` decides every probe host through `decide_landing`, so never-touch
  hosts and the window's own origin count. The probes are every exact pattern, one fresh label
  under each suffix, and that label alone; the label has no dot and is renumbered until no probe it
  builds is an exact pattern. A probe a ceiling refuses counts neither as admitted nor as narrowed
  by a layer, so a never-touch host stays a ceiling line and does not turn "Available" into "Some
  blocked". A never-touch probe still keeps each layer's own answer and the first layer whose list
  matched, so a layer whose only sites are never-touch is not counted as allowing it nowhere, and
  layers whose only shared sites are never-touch are not called disjoint. That line reads "Not
  available. The only sites X allow it on are marked never-touch by Y." and credits Y (review fix:
  the line had blamed the layer that allowed it). To know Y, each `PolicyLayer` now keeps its own
  never-touch list, so the layer-dropping flatten in `assemble` that P2 names is already deleted;
  P2 still credits the layer on the landing refusal itself. Three departures in shape, none in decision: `could_admit` asks
  the same probes and stops at the first admitted one instead of computing the whole reach, which
  is equal by construction and pinned by the agreement test; an unavailable line now names a layer
  that admits the capability nowhere on its own, not the first decider, which could be a layer
  that allows it somewhere; and the disjoint sentence ("The sites X and you allow do not overlap.")
  sits in `effective.rs` beside the others until P8 moves them. `note()`'s "no effect" check reads
  the snapshot over the rule's hosts, which also stops a wide rule under a narrow organization
  grant being marked inert. Dropping `browser_screenshot` when screenshots are off is C10c's. The
  catalog and result goldens are unchanged.
- P2: a host a layer marks never-touch is refused as that layer's policy decision: reason
  `protected_host`, rule `sacred_host`, the first matching layer's tier and no grant, and a denial
  id taken from that layer's hash and the rule alone, so every host one layer lists shares one id
  and the id names none. The audit line's `policy_tier`, `policy_rule` and `denial_id` and the
  result's facts carry it, and the window's refusal line names the organization from
  `policy_tier`. The layer-dropping flatten was already gone with P1, so nothing else is deleted;
  `probe` now reads the never-touch author from the decision instead of parsing the URL again.
  Two departures in reading, none in decision. The permission check's `layers` stay empty for a
  never-touch refusal: no grant was evaluated, and `language::history::permission` (core's) would
  otherwise read "no matching grant" or "no capability grant was required", both false. The credit
  rides the decision's attribution. And Decision 7.2's "localhost, loopback and link-local stay
  Ghostlight's own ceiling" is not restored: ADR-0155 removed those ceilings on the owner's
  direction, so only non-HTTP(S) schemes (and the window's own origin, ADR-0199) stay unattributed.
  The SIEM guide's `sacred_host` is D2's. The catalog and result goldens are unchanged.
- P3: `AuthoritySnapshot::replay(channel, requirements, host)` takes the record's channel as an
  `Option`, so a line that names none (one written after its workspace was gone) is read as MCP
  work, and simulate calls it. `decide_record` is deleted. The proof replaces
  `simulate_refuses_the_recorded_requests_of_a_channel_the_candidate_turns_off`: it now also holds
  that an undeclared line is refused with the MCP one. No departure. The window's preview still
  decides by host alone until S4a moves it onto `replay` with each item's channel.
- P4: `OrganizationIdentity::resolve` is the one precedence: the signed presentation's name,
  rationale and contacts win, and the manifest's block fills what it lacks (its URL always, since a
  presentation carries none). A contact list is taken whole from the carrier that has one. A signed
  publication is resolved once, when it is verified, and kept on the managed authority's active
  policy; the unsigned managed file (P6's) resolves from its block alone. The snapshot carries the
  identity, and `channels_off`, the chip and the compiled view read it from there. The passport is
  named from the same identity, by the managed authority (so the status file) and again at
  publication (`ManagedPolicyPassport::naming`), so the refusal row, which reads the passport,
  names a manifest-only organization. `effective::identity` is deleted, and the passport no longer
  reads the presentation alone. One departure in shape, none in decision:
  `OrganizationIdentity.name` is now optional, because a valid presentation may give a rationale
  and contacts without a name, and ADR-0122 Decision 3 keeps such bundles behaving as they did; the
  window's `authority.organization.name` can therefore be null, which every page reader already
  falls back from. Cross-lane: surface's `workbench/parts/tests.rs` fixture wraps its name in
  `Some`. The proofs live in `governance/managed/mod.rs`, beside the signed-bundle helpers. The
  catalog and result goldens are unchanged.
- P9: `migrate::migrate_file` takes an `Access`: `OwnerOnly` writes the moved file through
  `atomic_replace`, `Kept` through `atomic_replace_keeping_access`. `read_policy` moves an owned
  file with `OwnerOnly`; `ghostlight policy migrate` keeps `Kept`, because it moves a file the
  person names, which other accounts may read. No departure. The proof writes the schema-3 file
  open first (0644 on Unix, the directory's inherited access on Windows) and checks the moved file
  and its backup are owner-only. The catalog and result goldens are unchanged.
- P5: `GovernanceFacade::published()` is public and returns the whole `Publication` (renamed from
  `Published`), with its snapshot, compiled answer, chip and diagnostics. The chip is
  `PolicyChip::of(&authority)`: its situation and sentence are the compiled view's, and its warning
  tone reads the view's passport. The situation match in the publication build and `effective::chip`'s
  own headline call are deleted. The audit requirement is resolved once, at assembly, over the
  snapshot's layers (`AuditRequirement::of`); enforcement and the compiled view read it, so
  `AuditPolicy::resolve`, `requiring_layer` and its always-true `matches!` are deleted. The
  publication's own `startup` copy is gone too: `browser_startup()` reads the compiled view.
  `EffectiveAuthority.id` carries the snapshot's id. Three departures in shape, none in decision.
  The id is not serialized: it digests the layers' documents, and the window withholds such a
  value from a watcher, as it already does for the policy hint's tag. The policy hint the person's
  own write sends is now tagged with that id, as the refresher's already was (it had carried the
  user layer's edit tag, so one part's tag meant two things); the write precondition still uses the
  edit tag. Cross-lane, surface's: `workbench/live.rs::policy_changed` reads one publication, the
  hint test in `workbench/tests.rs` becomes `the_authority_id_tags_the_policy_hint`, and
  `workbench/web/tests.rs`'s hint check names the authority id; S3 and S4a had not started in those
  files. The executor keeping the publication and `policy_explain` reading it are C10c's, so the
  facade's single-fact readers stay for their callers. The proofs `the_chip_is_the_compiled_headline`
  and `enforcement_and_the_view_name_the_same_audit_layer` live in `effective.rs`. The catalog and
  result goldens are unchanged.
- P6: `GovernanceFacade::new` takes the user policy only, and `owning_user_policy` takes its path
  only. `PolicySources.managed` and every branch that read it are deleted, so the organization
  layer, its identity, its validity and its passport come from the managed authority alone; a
  facade built without one reports the new `ManagedPolicyPassport::not_configured`. With one file
  left, `PolicySource` loses its tier parameter. The organization layer's source line is always the
  passport's class ("Signed bundle from a local file"), never a path. The layered tests publish
  their organization layer through `ManagedPaths::signed`, a test helper that signs one local
  bundle and writes its bootstrap, under `with_managed_paths`, which now takes an optional user
  policy (so `PolicySources::remote_only` is deleted too). Two departures in shape, none in
  decision. The fail-closed proofs live in `governance/managed/mod.rs` beside the bootstrap
  helpers: `invalid_managed_authority_fails_closed` (a malformed bundle),
  `an_unreachable_managed_authority_fails_closed_from_cold_start`,
  `a_signed_schema_three_manifest_fails_closed_and_its_bundle_is_never_rewritten` and
  `a_passport_never_reports_an_unsigned_file_as_verified` (an unsigned manifest as both source and
  cache). The administrator schema-3 test kept only its user half, as
  `a_named_schema_three_user_policy_fails_closed_and_is_never_rewritten`. And the call sites were
  about 150, not twenty: every core test and surface test (`workbench/tests.rs`,
  `workbench/web/tests.rs`, `workbench/parts/tests.rs`) that built a facade, edited mechanically;
  no lane had an open slice in those files. The catalog and result goldens are unchanged, and
  `tests/golden/agents/native/linux.txt` is not affected.
- P7: `manifest::PolicySetting` is the closed vocabulary of `config[].key`, and `key()` spells each
  key once. `validate_config` decodes the key through it and each setting checks its own value;
  `assemble`, `PolicyLayer::new`, the screenshot and channel checks, the audit requirement and
  `Manifest::browser_startup` read through it, and the `Manifest` readers take a `PolicySetting`
  instead of a string. `SCREENSHOTS_ENABLED_KEY`, `MCP_CHANNEL_ENABLED_KEY`,
  `CLI_CHANNEL_ENABLED_KEY`, `audit::MODE_KEY` and `channel_enabled_key` are deleted;
  `PolicySetting::channel_enabled` takes the last one's place. One departure in shape, none in
  decision: `ConfigEntry.key` stays the authored string, so an unknown key is still refused at
  `config[i].key` with "is not a registered policy setting" rather than as a serde shape error, and
  the document is written back byte for byte. The proof
  `every_registered_setting_parses_and_round_trips` pins the eight schema-4 keys literally, parses
  one document authoring all of them, reads each back through its reader, writes it back unchanged,
  and refuses a wrong-typed value for each. Test fixtures that author documents keep their literal
  JSON, since they are documents. Cross-lane, core's: `work/tests/channels.rs` and
  `work/tests/documents.rs` name the key through `PolicySetting`; no core slice was open in them.
  `workbench/parts.rs`'s `SACRED_DOMAINS_KEY` and the page's copy are S4b's. The catalog and result
  goldens are unchanged.
- P8: `language::policy` holds every sentence the compiled view, the facade's authoring refusal
  and `ghostlight policy` say: the capability labels, the situation headline, each capability
  line (including P1's disjoint and never-touch lines), the layer titles, the ceilings, the signed
  source classes, and the validate, explain, simulate and usage lines. `effective.rs`'s `wording`,
  `layer_word` and `situation_sentence`, `passport_source`, the `AuthoringError` literals and
  `inspection.rs`'s `explain` and `short_hash` are deleted. "Not owned" and "no home" are one
  sentence each, `policy::NOT_OWNED` and `policy::NO_HOME`: the window's "shows it without changing
  it" and its "This environment names" give way to the error's wording. One departure in reach,
  none in decision: one sentence was not enough while the window and the write decided apart, so
  the window's user layer now reads the facade's own answer (`PolicySources::writable_user_path`,
  which the write reads too) instead of re-deriving it from the layer's source and
  `paths::user_policy_path()`. A file an environment variable names but that does not exist no
  longer shows an editor whose every write would be refused, and `user_layer.path` is absent
  whenever the window may not write. `OWN_CONTROL_PAGE_CEILING` stays in core's
  `language/outcome.rs`, which `policy::ceilings` reads. The proof
  `the_window_and_the_error_say_one_sentence_for_a_policy_this_window_cannot_change` lives in
  `governance/tests.rs` and covers a named file, an absent named file and a machine with no home.
  The catalog and result goldens are unchanged.

## The second sweep (2026-10-20)

Rules: [BOOTSTRAP.md](BOOTSTRAP.md), "The second sweep (2026-10-20)". Machine: test-03 (Alpine), after core-work. A
slice's specification is its finding in [sweep-2026-10-20/work.md](sweep-2026-10-20/work.md), read
with its skeptic's corrections. WORK-15, WORK-11 and P-lock have landed; this lane's second-sweep
rows are done.

| Slice | Subject | After | Source | Status |
| --- | --- | --- | --- | --- |
| WORK-15 | refactor(governance): one `HostPattern` parsed at load, one `url_host` and one `layer_denial` | - | WORK-15 as corrected; `denial_id` bytes stay | landed (ddd76f9c, 86013a28, cacb1062) |
| WORK-11 | refactor(governance): one internal word for the never-touch rule, `AuthorityTier` folded into `PolicyAuthor`, and `governance::audit` renamed | - | WORK-11 as corrected; persisted words stay | landed (ed389024, 5dcd3eaf, b82ff05d and the re-export) |
| P-lock | refactor(governance): the crate `lock` helper here | SERV-7 | SERV-7's adoption | landed (d00a7ede) |

WORK-15, `HostPattern`: `governance/host_pattern.rs` holds the grammar (`*`, `*.suffix`, an exact
host), parsed once and lowercased. `PolicyLayer::new` parses its never-touch list and each grant's
host rules when the layer loads and keeps the authored never-touch strings for what the Policy page
shows. `valid_host_pattern`, `pattern_specificity`, `host_matches` and `pattern_covers` are gone;
validation, grant evaluation, the never-touch match, the probe set, `permits_any_document` and the
Policy page's shadow check read the parsed patterns. The manifest's own check that a never-touch
list holds no `*` stays on the raw text, before parsing. The parse has its own mixed-case test.

WORK-15, `url_host`: `host_pattern::url_host(&Url)` is the lowercase host every pattern compares.
The never-touch match, host admission and the decision evidence read it. Three more copies are in
core-work's files and adopt it there: `observed_host` (`work/mod.rs`), `TabHold::host`
(`workspace/mod.rs`) and `reusable_site` (`workspace/released.rs`).

WORK-15, `layer_denial`: the six policy refusals (a grant's, the screenshot, channel, never-touch,
tab-close and audit-availability settings) go through `AuthoritySnapshot::layer_denial`, the one
place `Decision::policy` and `denial_bytes` are written. It derives the layer's document hash and
the deciding grant's id itself; a layer the snapshot does not hold digests the snapshot's id, the
tab-close fallback, which no other site can reach. `every_policy_refusal_keeps_its_denial_id` pins
the six persisted ids, captured from the code before the change.

WORK-11, the tier fold: `AuthorityTier` folds into `PolicyAuthor`, which had the same two cases,
as the skeptic corrected (`LayerKind` has a third, `Ghostlight`, that no layer can be). Its
persisted words come from one `tier_word`, beside `MANAGED_TIER` and the new `USER_TIER`, and stay
`managed` and `user` in audit records and the authority id. `channel_refusal`'s hand-written match
goes.

WORK-11, the never-touch word: `PolicySetting::NeverTouch`, `PolicyRule::NeverTouch` and
`never_touch_hosts` (the snapshot's list and the effective view's input) join the `never_touch`
names the code already used. The persisted words are unchanged: the key
`content.security.sacred_domains`, the rule word `sacred_host` and the reason `protected_host`.
`ReasonCode::ProtectedHost` keeps its name, because it also refuses every non-HTTP(S) address.

WORK-11, the rename: `governance::audit` is `governance::audit_requirement`, so the
audit-availability rule no longer shares a name with `crate::audit`, the writer. The module doc of
`governance` names what it holds and no longer promises "minimized audit intent". Left: dropping
`governance`'s re-export of `crate::audit::{AuditSink, JsonlAuditSink}`. Its users are
`service/mod.rs` (core-service), `work/mod.rs`'s tests (core-work) and `workbench/tests.rs` and
`workbench/history.rs` (surface), so it lands as one cross-lane commit when those lanes have no
open slice there.

P-lock: governance's private `lock` and its two direct locks of the observer read
`crate::sync::lock` (SERV-7, 0ba38fa7). It had no `Condvar` wait.

WORK-11, the re-export: `governance` no longer re-exports `crate::audit::{AuditSink,
JsonlAuditSink}`, so the writer has one path. `service/mod.rs`, `work/mod.rs`'s tests and
`workbench/tests.rs` and `workbench/history.rs` import it from `crate::audit` (cross-lane: the
coordinator's leave for `service/mod.rs`, test-04's hold for the two workbench files).
