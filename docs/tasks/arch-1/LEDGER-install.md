# arch-1: install ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Every arch-1 slice has landed; I11 landed with follow-up F2 after the owner's answer to question 1. The
follow-up slices of 2026-10-19 are recorded under Notes, Follow-up (2026-10-19).

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| I1 | fix(install): one owner-private creator for secrets and records | candidate | none | see Notes | landed (this commit) |
| I3 | fix(install): Ghostlight owns the command, and the client keeps its own keys | candidate | ux-2 closes (`install/mod.rs`) | see Notes | landed (this commit) |
| I7 | fix(deployment): a same-version install restores a missing shell | candidate | none | see Notes | landed (this commit) |
| I9a | docs(plugin): the plugin README withdraws the precedence claim | candidate | none | see Notes | landed (this commit) |
| I9b | fix(plugin): the skill says only what the catalog cannot | candidate | none | see Notes | landed (this commit) |
| I11 | fix(plugin): the plugin registers what question 1 decides | candidate | owner question 1 (the plugin) and the live check | owner's answer 2026-10-19; see Notes, Follow-up | landed (F2, this commit) |
| I4 | fix(install): one rule says Ghostlight has an entry to remove | none | ux-2 closes (`install/mod.rs`) | see Notes | landed (this commit) |
| I5 | fix(install): Ghostlight's npm launcher entry is Ghostlight's | none | none | see Notes | landed (this commit) |
| I2 | fix(install): one atomic replace and one same-location rule | none | I1 | see Notes | landed (this commit) |
| I6 | fix(install): the roster cache follows the acts, and a command reads it once | none | ux-2 closes (`install/mod.rs`) | see Notes | landed (this commit) |
| I8 | fix(packaging): the carriers agree with the service's rule and words | none | none | cross-lane the installer journeys; see Notes | landed (this commit) |
| I10 | refactor(install): install words in language/deployment, and the browser list composed once | none | ux-2 closes (`install/mod.rs`) | cross-lane `language/outcome.rs`; see Notes | landed (this commit) |
| I12 | refactor(diagnostics): one spelling of the edge's logs, and no name nobody writes | none | none | see Notes | landed (this commit) |
| I13 | refactor(win-peer): one token module for its two callers | none | none | see Notes | landed (this commit) |

## Notes

- I1, harness locations: they are written through `record::atomic_replace`, not the client-config
  writer, so every write is owner-only and the file gets no `.ghostlight-backup` copy beside it.
  On Windows that copy would take the directory's access. A locations file written before this
  commit becomes owner-only at its next write, as `install.json` does; nothing is re-protected at
  start. Not a changed decision.
- I1, the Windows proof: the orchestrator forbids `unsafe`, so the tests read the access list with
  the system's `icacls /save` and the user's SID with `whoami /user`. win-peer gains no new call.
- I1, for C19b: `install::record::create_private` is now public, for the audit sink's new log.
- I1 changes no golden and needs no journey. `linux.txt` is unaffected.
- I3, the owned `type`: the one current predicate checks the dialect's `type` with the command
  and its arguments, so an entry with the right command and no `type` (or another one) is now
  updatable where it was installed, and an install sets only that key. A `type` in a dialect that
  needs none is the client's. Not a changed decision: Decision 10.1 names `type` an owned key.
- I3, the written bytes: an owned entry is updated key by key, so only owned keys that differ are
  set and missing owned keys and defaults are appended. A one-line stale entry stays on one line;
  a one-line entry that gains a key (`type`, `env`, `enabled`) is laid out one key per line, in its
  own order, not the alphabetical order of a whole new entry. The writer golden pins each case. A
  current entry spelled another way (case, `.` or a link to the same edge) is now left alone,
  because the install reads inspection's path rule.
- I3, the Codex writer already edited its entry in place; only its equality no-op went.
- I3, an extra proof: `a_dialect_that_needs_a_type_is_updatable_without_it_and_the_update_keeps_the_rest`.
- I3 changes no native golden: they seed no Ghostlight entry, so `linux.txt` is unaffected. It
  needs no journey.
- I7, the launchers: they check the shell only when the installed record is this package's own
  version. A newer installed set whose shell is missing still runs as it is, because the service
  refuses to install an older payload over it; installing there would turn a missing window into
  a launch that fails. A changed shell is not seen by the launchers, which never hash the
  installed set; the service's same-version install finds it. Not a changed decision.
- I7, the service: `shell_changes` is read once per plan and passed to `place_in_bin`, so a
  same-version plan with a missing or changed shell takes the place-in-bin arm and restarts, as a
  damaged service image does. An extra proof covers the second `Same` arm (a retired development
  directory left to remove): `a_same_version_install_with_a_missing_shell_and_a_migration_restores_the_shell`.
- I7 changes no golden. It needs no new journey: the installer journeys on a test machine cover
  the place-in-bin path it reuses. `linux.txt` is unaffected.
- I9a, the double registration: the README's client note now says only that Claude Code then holds
  two registrations named `ghostlight` and that which one it starts is unverified. I11 replaces
  that line with question 1's answer and the live check. Not a changed decision.
- I9a, the network sentence: "the only network traffic is the binary download" was false too, since
  `npx` asks the npm registry at every launch. The README now says so and points to the npm
  launcher row of `docs/trust/data-flows.md` for when the launcher downloads. Within the slice's
  Changes.
- I9a changes no code, golden or journey. `linux.txt` is unaffected.
- I9b, the restated sentences: the skill no longer says a document inspect returns a `snapshot_`
  handle or that navigating through a closed tab handle recreates the tab. Both sentences went
  because they restate what the catalog and results already say (the slice's Deletes), not
  because they were false: both behaviours hold in 2.0 (`register_snapshot` in
  `work/reading.rs`; the `StaleTab` recovery in `work/navigation.rs`). Commit 2cd2131d's body
  calls them claims; read it as this note says. The skill also drops the retired "session hold"
  and "Pause/Stop" (G4-02): a credential refusal now says nothing is held or paused. Its result-field list is gone; it speaks of what a result says
  instead.
- I9b, the hold sentence: the skill says a refused request does not pause the session. It drops
  "later permitted requests remain available", because a tab held after a refused landing refuses
  them until the hold clears (C6). Not a changed decision: Decision 5.5 keeps "do not ask the user
  to resume".
- I9b, the plugin README: its last sentence said the skill is kept in lockstep with LANGUAGE.md. It
  now says the skill restates nothing of the catalog. Within the slice.
- I9b, for C20: the body reads clean through a scratch port of `hint_violations` against
  `docs/1.0/catalog.json`. Tool names are written bare and handle prefixes unquoted, as the catalog
  writes them; the one backticked word is `user_authorized_credentials`. The YAML front matter
  (`name:`, `description:`) is not a hint, so C20 reads the body after it.
- I9b changes no code, golden or journey, and no plugin version (D1's RELEASE.md step bumps it).
  `linux.txt` is unaffected.
- I4, the one rule: `HarnessState::holds_owned_entry` (installed or updatable, the states inspection
  shows every current, updatable or retired registration as) and `HarnessSummary::has_entry_to_remove`
  (that, or an owned other copy). `can_uninstall` is the second where the configuration may be
  changed, and `left_private` reads it for a removal. The dry run's `client_would_remove` read its
  own installed-or-updatable test; it now reads the same method. Within the slice.
- I4, an extra proof: `every_owned_registration_is_shown_as_a_state_that_holds_an_owned_entry` ties
  the state rule to `holds_owned_entry`'s reading of a file.
- I4, the writer golden (`GHOSTLIGHT_AGENTS_GOLDEN_UPDATE=1`): 169 `offers:` lines in 21 dialects
  gain uninstall, every one under `state: updatable` (the stale, legacy relay, retired and
  owned-with-the-client's-keys seeds). The native goldens seed no Ghostlight entry, so `linux.txt`
  is unaffected. No journey is needed.
- I4, for surface: the window still shows Remove only for an installed client
  (`ui/lib/view.js`, `actsOf`, `agent.state === "installed"`), so it hides a Remove the service now
  offers for an updatable one. That file is surface's and the slice's Changes do not name it, so
  surface's S9 drops that test (LEDGER-surface.md).
- I5, the state: every launcher entry is shown as `needs_attention`, with its own cause
  (`Attention::Launcher`), detail, evidence and dry-run line, and Fix offered. The window has no
  state between connected and needs attention that the reconcile, the setup of detected clients
  and Uninstall all leave alone, and a new `HarnessState` would cross into surface's window. So
  the unpinned entry is never current, as Decision 10.3 says, and shares the 1.x entry's state;
  only its words differ ("npm chooses the version at each launch"). Not a changed decision.
- I5, a third pin: an entry pinned to 2.0 or later (`LauncherPin::Pinned`) is not current either:
  its detail says Ghostlight cannot ask npm what that runs. The ADR names only the unpinned and
  the 1.x cases. Within Decision 10.3.
- I5, what is the launcher: `npx` (or `npx.cmd`, through `cmd /c` or not) with only quiet options
  (`-y`, `--yes`, `-q`, `--quiet`), then `ghostlight` or `ghostlight@<spec>`, then nothing or
  `mcp`. A spec with a character no version, range or tag uses (a URL, a path) is not the
  launcher and stays foreign, so the evidence can name the package and its version without
  disclosing anything else. Codex's TOML is read the same way.
- I5, the refusals: an explicit install or uninstall that meets the launcher now refuses with
  `HarnessError::LauncherEntry` (its words in `language/deployment.rs`), not the foreign-entry
  refusal, and Fix over it says it now starts the installed Ghostlight instead of the npm
  launcher. The launcher is not an owned entry (Decision 10.2), so Uninstall leaves it. Within the
  slice's Deletes.
- I5, the Fix test: `explicit_fix_replaces_only_the_foreign_entry_and_keeps_a_backup` seeded the
  npm launcher as its foreign entry; it now seeds another program, and the launcher's Fix is
  proved in `a_launcher_pinned_to_1x_needs_attention`. Extra proofs:
  `a_launcher_pinned_to_2x_is_never_current`, `only_ghostlights_own_package_through_npx_is_the_launcher`,
  `a_codex_launcher_entry_is_the_launcher_too` and, in `language/deployment.rs`,
  `a_launcher_entry_is_said_as_ghostlights_and_never_as_current`. The reconcile test
  (`an_owned_client_entry_is_rewritten_and_an_npx_entry_is_left_alone`) is unchanged and passes.
- I5, for surface: the window words a `needs_attention` agent with evidence itself
  (`ui/lib/words.js`, "Something Ghostlight did not write sits where its entry belongs"), so for
  the launcher it shows that line beside the service's evidence. That file is surface's; S5, which
  moves the window's words into `language/`, can word the launcher from `detail`.
- I5 changes no golden (no writer or native seed holds a launcher entry) and needs no journey.
  `linux.txt` is unaffected.
- I2, the one replace: `install::record::Replace` with three stated options, `access` (`Private`
  or `Kept`), `backup` and `link` (`Follow` or `Refuse`). `Replace::OWN` (private, no backup, a
  link refused) is `atomic_replace`; `Replace::SHARED` (access kept, no backup, a link written
  through) is `atomic_replace_keeping_access`. The client-config writer is `SHARED` with a backup,
  the native-host manifest and the policy bundle are `SHARED`, and the Linux desktop entry and the
  Flatpak override restore are `SHARED` with a link refused (inspection already calls a link
  there foreign). Every temporary is `.ghostlight-<uuid>.tmp` beside the file it replaces, and
  the stray sweep reads `record::is_temporary`.
- I2, links: `atomic_replace` used to rename over a link, leaving a plain private file where the
  link was. It now refuses the link (`language::deployment::link_refused`) and changes nothing,
  because the options are follow or refuse, and a private record must never be written through a
  link. `atomic_replace_keeping_access` used to do the same; it now writes through the link, so a
  policy file a person or tool keeps as a link stays one. No caller outside install changed. Not
  a changed decision.
- I2, the remove-then-rename fallbacks: every one went, not only "the second". The native host's
  went with `write_file_atomic`, and the client-config writer's and the desktop entry's went
  because the one replace has none: on Windows a rename that fails over a file also fails to
  delete it, and on Linux the rename never fails that way, so the fallback only lost atomicity.
  `HarnessError::InvalidPath`, `NativeHostError::InvalidPath` and
  `DesktopIntegrationError::InvalidPath` went with their writers; a path with no parent is now the
  replace's `InvalidInput` error.
- I2, the same-location rule: it lives in `install::entry` beside `canonical_path`. `same_path`
  compares spelling (on Windows without regard to case, slash direction, a trailing separator or
  the verbatim prefix), `same_location` adds the comparison once links are resolved, and
  `lies_within` says a path is below a directory. The crate-private `spelled_alike`,
  `same_location_on` and `lies_within` take whether the paths are Windows paths, so the tests that
  model a Windows computer on Linux keep working. Gone: `install/mod.rs`'s `comparable` and
  `paths_equal`; `migration.rs`'s `comparable`, `comparable_text`, `same_path`, `same_location` and
  `is_inside`; `native_host.rs`'s `normalize_path` and `same_path`; `apps.rs`'s `comparable` and
  `windows_entry.rs`'s. Case now folds by Unicode lowercase everywhere, as four of the old rules
  did. `user_path.rs` keeps its own compare: it reads `PATH` text with `%VARIABLE%` references,
  not paths.
- I2, which half each site reads: a path a client or another tool wrote (an entry's command, a
  legacy relay root, a registry value, a manifest's program) is compared by `same_location`; the
  copies of a client's settings, the Store app's private view and the home roots, which Ghostlight
  spells from the same roots, are compared by spelling, as before. Resolving them could merge a
  Store app's private copy with the plain file when Ghostlight runs inside that app's container.
- I2, the proofs: `a_bundle_write_is_synced_before_its_rename` reads the steps the replace took
  through a test-only per-thread log (`record::steps`). `register_and_inspect_agree_on_a_respelled_registry_value`
  respells the value through `.` on every system, and on Windows also with forward slashes and
  capitals; it runs on disk with an in-memory registry. `a_legacy_relay_entry_spelled_with_forward_slashes_is_owned`
  is Windows-only, since Linux reads a backslash as a file-name character. Extra proofs in
  `record.rs`: the one temporary pattern, a kept-access first write and its directories, the
  backup, and a link refused or followed. The native host's link test was renamed
  `a_registration_file_is_written_through_a_link` and goes through the system writer.
- I2 changes no golden and needs no journey. `linux.txt` is unaffected.
- I6, the lock: `refresh` takes the action lock and calls the private `scan`, which `apply`,
  `setup_detected` and `locate` call under the lock they already hold. `refresh` keeps its
  `Result`, though it never fails, because the window's `refresh_harnesses` reads it; changing it
  would cross into surface's file for no behaviour. `apply` finds its definition through
  `definition`, the one lookup by id.
- I6, the re-symbol call: `HarnessRegistry::resymbol` gives every cached row its agent's symbol as
  the icon cache now has it, under the action lock, and reads no client file. S3 makes the window's
  `icons_changed` (`workbench/mod.rs`, surface's) call it; until then an icon change rescans, now
  under the lock.
- I6, the commands: `ghostlight install` and `uninstall` build the client registry once, at the
  first need (the check of named `--client` ids, else the client step), and read its roster; only
  each act reads the clients again. The client step is now `cli::setup::run_client_steps`, so the
  proof can drive it over an isolated registry. Stray temporaries are now removed after that one
  read, not before it; inspection never reads them. `doctor` reads the roster its registry built.
- I6, `Registrations`: the client registry is built at the first `pending` or `reconcile`, so
  `install`'s and `uninstall`'s Windows-entry calls read no client file. A deployment still reads
  the clients twice, at the plan's `pending` and at the reconcile after `bin` changed; both are
  their own moment. Not a changed decision.
- I6, the proofs: `a_background_scan_never_overwrites_an_acts_result` holds the action lock as an
  act does and shows a background `refresh` waits for it, then stores the act's result.
  `one_install_scans_once_plus_once_per_write` counts scans through a test-only counter: one when
  the registry is built, then one for each of three writes.
- I6 changes no golden and needs no journey. `linux.txt` is unaffected.
- I8, the order: both launchers parse a version by `install/record.rs`'s rule (digits with no
  leading zero, fields within 64 bits, no empty pre-release identifier, build ignored) and order
  it by semver precedence, so `2.0.0-rc.10` follows `2.0.0-rc.2` and a version the service cannot
  read is unordered on both sides. The shared vectors are `packaging/version-order.json`, which
  neither carrier ships; `record.rs`'s two version tests and both launcher tests read it, and their
  inline vectors went.
- I8, the restore words: the launchers keep their own `RESTORE_ROUTE`, because they run before any
  service. `both_launchers_say_the_way_back_in_the_services_words` reads both files and compares
  their route lines with `restore_route()`, and their missing-edge sentence with
  `development_executable_missing`.
- I8, the get scripts: after `deployment install`, the same downloaded service runs `install`.
  The entry rule finds the installed service is the same image and forwards to it, so the summary
  still prints in the person's console; over a development build it forwards as the edge did. The
  `~/.ghostlight/bin` literal went, and with it `get.ps1`'s `USERPROFILE` check: the service decides
  where the home is. So the `GHOSTLIGHT_NO_REGISTER` sentence names no directory. When `install`
  fails, the `ghostlight` command may not exist yet (`install` creates it late, and on Linux
  `~/.local/bin` may not be on `PATH`), so the script runs the same downloaded service's `doctor`,
  which runs from anywhere and reads the same home, before the temporary directory goes, and then
  says only that installation did not complete. Within the slice's Deletes.
- I8, the npm staging: `scripts/prepare-npm-package.ps1` copies `packaging/version-order.json`
  beside the staged `package/`, where the staged launcher test finds it at the same relative path
  as in the source tree and `npm pack` never includes it. Shown by staging fixture artifacts with
  the script: the staged test passes and the tarball holds only the package's listed files.
- I8, the journeys (core's, named cross-lane): `tests/installer-shell.mjs`'s fixture service
  records which copy ran each command, the journey expects the downloaded service's
  `deployment install` and then its `install`, and the fixture edge now fails if it is run. A case
  whose fixture `install` fails expects the downloaded service's `doctor` to run and the one
  sentence "ghostlight: installation did not complete"; a Windows copy of that journey (Git Bash,
  shims for `uname`) passed here and failed on the earlier `get.sh`. `tests/installer-powershell.ps1`
  checks the `GHOSTLIGHT_NO_REGISTER` sentence; its fixture runs the real built images with
  registration off, so it cannot fail `install` without writing the machine's registrations, and
  `get.ps1`'s failure branch, which mirrors `get.sh`'s, is owed a live check. Neither ran here:
  the first is Linux-only and the second starts its own service. The coordinator runs
  `node tests/installer-shell.mjs` on Linux and `pwsh -NoProfile -File tests/installer-powershell.ps1`
  on Windows from a fresh build in `GHOSTLIGHT_BIN_DIR`, and `get.ps1` and `get.sh` once on a test
  machine without `GHOSTLIGHT_NO_REGISTER`, to see `install`'s summary reach the console through the
  downloaded service. Ran here: both launcher tests, `tests/installer-checksums.ps1` offline in
  PowerShell 7 and 5.1, and `sh -n scripts/*.sh`.
- I8 changes no golden. `linux.txt` is unaffected.
- I10, what moved: the harness details, evidence, apply and locate messages, `setup_message`, the
  `HarnessError` texts, the container sentences (`NOT_VISIBLE`, the kept-private lines, the
  cannot-see lines, the registry-view detail and the uninstall refusal), the doctor lines, and the
  word tables of all five registries (`HarnessState`, `NativeHostState`, `CommandPathState`,
  `DesktopIntegrationState`, `UserAssetState`), whose `label()` methods went. `container.rs` keeps
  the commands the sentences name (`finishing_command`, `checking_command`, `removing_command`)
  and `NOT_VISIBLE_STATE`, a JSON state word. The Windows entry nouns moved with them. Within
  Decision 8.
- I10, what stayed: the native-host details and the other `NativeHostError` texts, the
  command-path and documentation details, the reconcile report's prefixes (`command link:` and
  the like) and the rest of `cli/setup.rs`'s lines are not on the slice's list and keep their
  words in place. A later slice can move them the same way.
- I10, "the service": only doctor's readiness line spoke of the authority; it now says "The
  running Ghostlight service". "The Ghostlight program" in the client details and refusals names
  the edge, not the service, so it stays.
- I10, `ghostlight native-host`: it now prints the report doctor prints (`Ghostlight program:`
  and one `Browser:` line per browser, with `browser_state`'s word), after its first line. Its own
  state table (`missing`, `current`, `updatable`, `owned by another installation`) went. No test or
  journey parsed the old lines.
- I10, the family list: `install::native_host::supported_browsers()` reads the browser table, and
  `language::deployment` composes `browser_ids` and `browser_families` from it. The literals that
  went: `NativeHostError::UnknownBrowser`, the three in `select_install_browsers`, the `--browser`
  help line and the outcome's `BrowserAbsent` step (core's file, named cross-lane). Every sentence
  reads byte for byte as before, so the man page (`packaging/linux/man/ghostlight.1`) and the
  catalog and result goldens are unchanged.
- I10, the Windows key: `WindowsEntryChange::nsis_removed` is now `other_removed`, because it is
  set for any Uninstall key whose command is not this home's, and the entry reads "replacing
  another installation's". Windows-only code, compiled and tested here.
- I10, the desktop entry: an owned entry that already starts this installation and lacks only its
  icon reads "starts this installation, but its icon is missing". Proved in `desktop_entry.rs`
  (`an_entry_that_lost_only_its_icon_is_not_said_to_be_older`) and `language/deployment.rs`.
- I10, the older layout: `edge_replaced(PreEdgeLayout)` says "bin still holds an older layout that
  kept its connectors there", and the doc comments of `pre_edge_layout` and `PreEdgeLayout` no
  longer say only a development build left it. For D2: `docs/1.0/ARCHITECTURE.md` line 97 still
  says "the layout of an earlier development build".
- I10, the proofs: `native_host_check_and_doctor_say_one_word_per_state` (`cli/setup.rs`) and
  `the_family_list_is_the_browser_table` (`language/deployment.rs`), with the moved word tests. The
  writer golden was regenerated with `GHOSTLIGHT_AGENTS_GOLDEN_UPDATE=1` and is byte-identical:
  the state words and details did not change. The native goldens are unaffected, so `linux.txt`
  is unaffected. No journey is needed.
- I10, a defect and its repair: `e97d4618` staged `language/outcome.rs` as a whole file, so it also
  carried core's uncommitted held-tab hunks (`Refusal::TabHeld { host }`, `held_tab_summary`, a unit
  `WorkspaceReason::TabHeld`, and a test of `TabHold`'s new fields) without their dependents in
  `audit.rs`, `self_check/hints.rs`, `workspace/mod.rs` and `work/`. `e97d4618` alone does not
  build. The fixup after it stages a hunk-filtered `outcome.rs`: C6's file plus only I10's two
  hunks (the `deployment` import and the `BrowserAbsent` step), so the held-tab host C6 gave the
  refusal is back. Core's hunks stay in the working tree for core's own slice. The fixup also
  drops a doc line the move of `argument_count_phrase` left above `registration_state`. The gate
  ran on an export of the fixup's tree, not on the shared working tree.
- I12, the loop: the bridge's `EDGE_LOGS` (the relay's `Component::EdgeMcp` and `EdgeBrowser`) is
  now public, and `cli/diagnostics.rs`'s `writers()` names the service, then each edge log as
  `relay::log` spells it. `diagnostics show` reads it for its "No ... records in range" lines.
  Proved by `show_names_the_service_and_each_edge_log_once`, an extra test. Within the slice.
- I12, the event names: the fifteen that went are the four `native_*` and `service_connect_failed`
  names, the eight `demand_start_*` names, `service_connected`, `service_disconnected` and
  `adapter_replaced`. The edge's `service_connected` and `service_disconnected` lines are spelled
  by `relay::log::Event`, and the recording stop reason `service_disconnected` is its own
  vocabulary; neither read the bridge's names.
- I12, the sink tests that birthed a sink as an edge component now birth it as the service, the
  only component that writes through it. `parse_window`'s doc line, which sat above
  `op_and_detail`, moved to its function.
- I12 changes no golden and needs no journey. `linux.txt` is unaffected.
- I13, the module: `win-peer`'s `token` owns `ProcessToken` (opened once, closed on drop), its
  `is_elevated` and `user_sid`, and one aligned size-probed read of a token information class
  that both use. `elevation.rs` went; `is_elevated` and `private_file` ask `token`.
  `private_file` keeps only the access list and `CreateFileW`, and reads `token::LocalMemory` for
  the descriptor block, so `LocalFree` is declared once. The relay's own token code stays
  separate, as the ADR says.
- I13, the elevation read now probes the size first, as the user read did, instead of passing a
  fixed-size buffer. Same answer; one path. Extra proof in `token.rs`:
  `the_process_token_says_whether_it_is_elevated_and_whose_it_is`.
- I13, the words: the manifest and lib doc name the crate's current jobs and no longer cite
  ADR-0160; `create_private_file`'s doc and `private_file`'s comments no longer speak of a
  runtime token file. No caller outside win-peer changed. It changes no golden and needs no
  journey. `linux.txt` is unaffected; the module is Windows-only and Linux compiles none of it.

### Follow-up (2026-10-19)

- F2 (lands I11), the owner's answer to question 1: the plugin must "work regardless", so plugin
  only, installer only, or both give one working Ghostlight and one set of tools. The plugin keeps
  its server and asks npm for Ghostlight 2 (`npx -y ghostlight@2` in both manifests), so it never
  starts 1.x over a 2.0 install and never jumps a major. The marketplaces serve `main`, which
  advances only at the cutover, so nothing public changes before then. The live check is
  linux-2's L2-04 (Claude Code 2.1.303: both servers start); the one toolset is core's F1, which
  the README's client note now describes in place of "not verified". The integrity script proves
  the pin: the plugin's server must be `npx -y <npm package>@<its major>`, read from
  `packaging/npm/package.json`. The plugin version is unchanged; RELEASE.md's plugin step raises
  it before the candidate commit. No code, golden or journey changes; `linux.txt` is unaffected.
- F3, Claude Code's own directory (linux-2's L2-04 side finding): Claude Code 2.1 reads its user
  servers from `.claude.json` in `CLAUDE_CONFIG_DIR` when that is set and not empty, else in the
  home (read from the installed Claude Code itself, not a table). The installer wrote only the
  home file, so a person with `CLAUDE_CONFIG_DIR` set never saw the entry. The roster now reads
  the variable from the environment of the process that looks, else, on Windows, from the
  person's stored environment (`HKEY_CURRENT_USER\Environment`). The fallback is for the
  workbench: the long-lived service keeps the environment of whatever first started it, which
  may be a client that filters its child's environment, so without it the window would set up
  and remove the home file. Proof: `claude_code_is_set_up_and_removed_in_the_file_claude_code_reads`
  sets up, finds and removes the entry in each location and never writes the other, and
  `claude_code_directory_comes_from_the_process_else_the_person` proves the process value wins,
  the stored one fills in (expanded when it holds references) and an empty one counts as none.
  The guide's agent table regenerates. No result, catalog or writer golden changes; `linux.txt`
  is unaffected (its context sets no variable). Limits: on Linux a variable set only in a shell
  startup file is invisible to a service its shell did not start, so there the workbench uses
  the home unless its starter passed the variable on (`ghostlight setup` and `doctor` run in the
  person's shell and see it); a machine-wide Windows value is not read; Claude Code's legacy
  `.config.json`, which it prefers when that file exists and is not empty, is not handled. No
  journey needed.
- F4, a turned-off copy (linux-2's Notes, seen for windows-claude): the profile scan counted a
  profile as holding the extension when its `Extensions/<id>` directory existed, so a disabled 1.x
  store copy made the executor's probe answer yes and the refusal said to open a window of the
  browser, which cannot help: a browser does not load a disabled extension. A profile now holds
  the extension only when no preferences file (`Secure Preferences`, then `Preferences`) marks it
  disabled: a `disable_reasons` list that is not empty or number that is not zero, or the older
  `state` of 0. An unreadable or oversize file marks nothing. With no other copy turned on, the
  probe answers no and, while setup is incomplete, the refusal names the store listing (where a
  disabled copy can be turned back on) and the Ghostlight window. The workbench's setup reads the
  same scan, so it no longer offers to open a browser whose copy is off. Proof:
  `a_turned_off_copy_does_not_hold_the_extension` (each spelling, in each file, beside copies that
  are on) and `a_browser_with_only_a_turned_off_copy_sends_the_person_to_the_store` (the probe's
  own question over an installed browser, and the advice it resolves to). Limits: with setup
  complete the refusal stays the plain one, as before. No golden, catalog or wording changes;
  `linux.txt` is unaffected. No journey needed.
