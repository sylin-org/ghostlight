# Second sweep, 2026-10-20: install

This sweep read `crates/orchestrator/src/install/`: 25 files, 28,741 lines, of which `mod.rs`
(6,346), `deployment.rs` (4,003), `migration.rs` (2,860), `native_host.rs` (2,627) and `icons.rs`
(2,177) are the largest. It was read-only, on `dev` at `8f6a43ec`. The production half of
`mod.rs` was read in full, the other files where a finding rests on them, and tests were skimmed.
Where a finding crosses into `governance/`, `service/`, `workbench/`, `cli/` or the window's `ui/`,
that code was read too. Nothing was edited, built or run. The writer golden
(`tests/golden/agents/writes/`, checked by `install::writer_golden`) and the native goldens
(`tests/golden/agents/native/`) pin every byte and sentence the installer writes into an agent's
MCP configuration, and `install::agent_list` pins the installation guide's agent table, so every
section rates its effect on installer output on its own line. The area meets these ADR-0203 slices
of the arch-1 install lane: I1 (one owner-private creator), I2 (one atomic replace and one
same-location rule), I3 (Ghostlight owns the command and the client keeps its keys), I4 (one rule
for an entry to remove), I5 (the npm launcher entry), I6 (the roster cache and `resymbol`) and I10
(install words in `language/deployment.rs`). It also meets these entries the ADR considered and
did not take: G3-05, G3-10 and V-09. No finding touches the development-layout shims the ADR keeps
until 2.x (A-11). Where a finding meets one of these, the section says so. Findings are ranked by
payoff over risk. Ratings give payoff, risk and effort (S, M, L).

## INST-1: Home file names are defined twice, once by the reader and once by the migration

Kind: magic literals, two owners for one persisted name.

Where:

- `harness-locations.json`: `HARNESS_LOCATIONS_FILE` (`crates/orchestrator/src/install/mod.rs:78`)
  and `HARNESS_LOCATIONS_NAME` (`crates/orchestrator/src/install/migration.rs:106`).
- `user-policy.json`: `USER_POLICY_FILE` (`crates/orchestrator/src/governance/paths.rs:14`) and
  `USER_POLICY_FILE_NAME` (`migration.rs:93`).
- `managed-policy.bundle` and `managed-status.json`: raw literals in
  `crates/orchestrator/src/governance/managed/mod.rs:54`, `55`, `63` and `64`, and constants in
  `migration.rs:96` and `99`.
- `audit.jsonl`: `AUDIT_FILE_NAME` in `crates/orchestrator/src/service/mod.rs:106` and again in
  `migration.rs:90`.

Problem: the 2.0 break migration moves each 1.x file to a name that its 2.0 reader defines on its
own. A rename on either side silently strands the migrated file where nothing reads it.
`install/layout.rs` already holds the other state file names (`SETUP_RECORD_NAME` at
`layout.rs:65`), and the doc of `STATE_DIRECTORY_NAME` (`layout.rs:44`) even lists "harness
locations", but these names are not there. The migration uses one name for the 1.x source and the
2.0 destination, so it can read the same constant.

Proposed change: declare these names in `layout.rs` beside `SETUP_RECORD_NAME`, with `Layout`
accessors such as `Layout::harness_locations()` and `Layout::user_policy()`, and make the readers
and the migration use them. ADR-0203 lists this as V-09: considered and not taken in arch-1, and
left "for whoever next touches that code".

Ratings: payoff high, risk low, effort S. Installer output: none.

Verdict: partial. Four of the five names hold: `user-policy.json`, `managed-policy.bundle`,
`managed-status.json` and `harness-locations.json` are spelled by their 2.0 readers and again by
the migration, which joins one name to the 1.x source and the 2.0 destination
(`install/migration.rs:759-793`). `audit.jsonl` is not that case. The migration never places a
log where the service reads it: it archives the 1.x log as `audit-<label>.jsonl`
(`migration.rs:794-802`, and `install/version_directories.rs:83-87` for `bin/v*`). So
`migration.rs:90` and `service/mod.rs:106` are a 1.x fact and a 2.0 fact that are spelled alike,
and they should stay two. The same split applies to the other four: give the migration's
destinations the `Layout` accessors and keep its 1.x source names its own, or a later 2.0 rename
would stop the migration finding the 1.x files.

Class: refactor, persisted.

arch-1: reopens V-09 (Considered and not taken: one table of home file names in this batch),
which the ADR leaves for whoever next touches that code. The names belong to the install home
layout, a seam ADR-0203 keeps, so every byte stays.

## INST-2: `install/mod.rs` is two modules

Kind: structure, stale module doc.

Where: `crates/orchestrator/src/install/mod.rs`, 6,346 lines: about 3,250 lines of production code
(lines 1 to 3253) and about 3,090 lines of tests (3254 to 6346). The module doc is line 1.

Problem: the file is the root of the `install` namespace (it declares the 25 submodules, lines 3
to 29) and also the whole harness feature:

- the registry, `HarnessRegistry` (`mod.rs:118`), its public types (`mod.rs:425` to `690`) and
  `HarnessError` (`mod.rs:3211`);
- the context and its roots, `HarnessContext` (`mod.rs:691`) and `harness_roots` (`mod.rs:869`);
- the roster, `AgentRow` (`mod.rs:988`) and `ROSTER` (`mod.rs:1120`), with the agent-catalog
  queries `crate::agent` calls (`agent_called`, `catalog_agent`, `runs_inside`, `monogram_of`,
  `icon_catalog` and `app_catalog`, `mod.rs:1704` to `1792`);
- inspection (`inspect`, `mod.rs:1794`) and npm-launcher detection (`NpmLauncher`, `LauncherPin`,
  `mod.rs:1959` to `2085`);
- the JSON, Codex TOML and manual YAML writers and their parsers (`mod.rs:2096` to `3061`),
  evidence bounding (`bounded_disclosure`, `mod.rs:2973`) and the located-path overrides
  (`mod.rs:3063` to `3100`).

The module doc says "Explicit, ownership-checked development-harness registration owned by the
orchestrator", which is wrong for the parent of deployment, migration, native hosts and icons.
Siblings reach into the harness root for shared helpers: `install/icons.rs:44` imports `lock`,
`HOME_VARIABLE` and `USER_PROFILE_VARIABLE` from it.

Proposed change: move the harness feature into `install/harness/`: `mod.rs` (the public types and
`HarnessRegistry`), `context.rs`, `roster.rs`, `json.rs`, `codex.rs`, `launcher.rs` and
`locations.rs`, with `writer_golden` and `agent_list` beside them. `install/mod.rs` becomes a short
index whose doc states the install tree's role. AGENTS.md ("Building and testing") and
`docs/1.0/ARCHITECTURE.md` name `install::ROSTER` and need the new path, or a re-export keeps the
old one. A later step can move `icons.rs` and `apps.rs`, which install nothing and serve
`crate::agent`, `crate::provenance`, `service` and `workbench`, under `crate::agent`.

Ratings: payoff high, risk low (moves only; private items become `pub(super)`), effort L.
Installer output: none.

Verdict: partial. The split holds: production code ends at line 3253, tests run from 3254 to
6346, and the doc at line 1 describes only the harness feature. Corrections: `mod.rs:3-29`
declares 24 submodules, not 25 (the 25 files include `mod.rs`). The catalog queries are called by
`crate::agent` (`agent_called`, `monogram_of`), `language/provenance.rs` (`agent_called`,
`catalog_agent`, `runs_inside`) and the siblings `icons.rs:647` and `apps.rs:153`
(`icon_catalog`, `app_catalog`), and `icons` and `apps` also serve `cli/doctor.rs` and
`language/agent_icon.rs`. "Installer output: none" holds only while the recorded names stay: all
26 files under `tests/golden/agents/` open with "Recorded by install::writer_golden", a literal at
`install/writer_golden.rs:1063` and `1287`, and the guide's table says it is generated "by
install::agent_list" (`install/agent_list.rs:273`, `docs/guides/installation.md:113`). Moving
those two test modules leaves the names false or rewrites every golden, `linux.txt` included, so
keep them in `install/` or rename the headers in a commit of their own.

Class: refactor.

arch-1: none.

## INST-3: One harness has seven names

Kind: naming, vocabulary drift.

Where, the seven names for one roster row and its id:

- harness: `HarnessSummary.id`, "Stable product-owned harness identifier"
  (`crates/orchestrator/src/install/mod.rs:490`), and the `Harness*` type family.
- target: `HarnessSetupFailure.id`, "Stable target id" (`mod.rs:682`); `HarnessSummary.target`,
  "User-facing concrete target label" (`mod.rs:499`); "every detected target" in the doc of
  `setup_detected` (`mod.rs:252`); "MCP target" in `crates/orchestrator/ui/lib/transport.js:493`
  and `496`.
- host: `ClientIds.hosts` (`mod.rs:618`) and `ClientName::Host`, "the host's own harness id"
  (`mod.rs:571` and `572`); the guide's "Host id" column (`install/agent_list.rs:276`,
  `docs/guides/installation.md:117`); the command-line help, "Select an agent's detected hosts, or
  one host" (`crates/orchestrator/src/cli/parse.rs:382`).
- client: `client_ids()` (`mod.rs:638`) and `ClientName`; `Registrations.clients` and
  `reconcile_clients(clients: &HarnessRegistry)` (`install/reconcile.rs:113` and `346`); "a model
  client's first request" (`install/setup_state.rs:3`); `harness_locations_directory`, documented
  as "Where the 1.x client location file was" (`install/migration.rs:463`).
- row: `AgentRow` (`mod.rs:988`), though its doc says it is one agent on one host (`mod.rs:984`).
- definition: `HarnessDefinition` (`mod.rs:899`), which is a row resolved on this computer, not a
  definition.
- product: `product_id`, "shared by coexisting targets" (`mod.rs:492`).

"Host" means two more things in the same tree: the app a harness runs inside, in
`runs_inside(agent_id, host_id)` (`mod.rs:1725`), and the browser's native-messaging host
(`install/native_host.rs`).

Problem: a reader meets one concept under seven names, and "host" under three meanings.

Proposed change, one glossary:

- agent: the catalog identity (`agent_id`, `crate::agent::Agent`).
- harness: one agent's MCP configuration on one host, that is one roster row. This already matches
  the wire (`harnesses`, `harness_act`, `{"harness": id}`) and the persisted
  `harness-locations.json`.
- host app: only what `runs_inside` and `icon_family` name.
- native host: only browser native messaging.
- "client" and "host" stay only in words people read: `--client`, `--all-clients`, the sentences
  in `language/deployment.rs`, the installation guide and the command-line help.

Renames: `AgentRow` becomes `HarnessRow`; `HarnessDefinition` becomes `ResolvedHarness`;
`ClientIds { agents, hosts }` becomes `HarnessIds { agents, harnesses }`; `ClientName` becomes
`NamedBy`; `client_ids` becomes `harness_ids`; `Registrations.clients` and `reconcile_clients`
become `harnesses` and `reconcile_harnesses`; `runs_inside(agent_id, host_id)` becomes
`runs_inside(agent_id, app_id)`. Doc comments that call a harness a target, host or client follow.

Ratings: payoff high, risk low (identifiers and docs only), effort M. Installer output: none. A
separate, golden-changing option: renaming the `HarnessSummary.target` field changes the workbench
wire key `harnesses[].target` (pinned in `crates/orchestrator/src/workbench/parts/tests.rs:571`)
unless serde keeps the old name, and renaming its words too would change the writer golden's
`target:` line (`install/writer_golden.rs:832`) and the guide's Target column.

Verdict: partial. Each cited spelling is there, but "seven names for one row" overcounts:
`product_id` is another fact (one product spans rows: Devin's is `windsurf`, `mod.rs:1241`, and
the four Cline rows share `cline`), `HarnessSummary.target` holds the host's label (User, CLI,
Cursor) rather than naming the row, and "a model client's first request" (`setup_state.rs:3`) is
a live MCP connection, not a row. `icon_family` names the vendor app whose icon a tool wears, not
an app it runs in, so "host app" fits only `runs_inside`. The renames are identifier-only, as
claimed. The glossary's merits, and the words a sweep must keep, are in the Skeptic summary.

Class: refactor.

arch-1: none.

## INST-4: Per-user directories are resolved nine times, under different rules

Kind: magic literals, duplicated logic, behavior drift.

Where:

- `HarnessContext::system` with `harness_roots` (`crates/orchestrator/src/install/mod.rs:749` and
  `869`).
- `NativeHostContext::system` with `native_host_roots` (`install/native_host.rs:325` and `406`).
- `LegacyRoots::from_environment` (`install/migration.rs:373`).
- `IconPlaces::system` (`install/icons.rs:313`).
- `DesktopIntegrationContext::system` (`install/desktop_entry.rs:101`), `UserAssetContext::system`
  (`install/user_assets.rs:178`), `BrowserPackageContext::system` (`install/browser_package.rs:58`)
  and `CommandPathContext::system` (`install/command_path.rs:168`).
- The Start-menu folder in `shortcut_path` (`install/windows_entry.rs:465`, reading `APPDATA` at
  `466`).

Problem: each resolves the home and the per-user roots on its own.

- `LOCALAPPDATA` is a constant three times (`mod.rs:89`, `install/container.rs:40`,
  `icons.rs:78`) and raw at `native_host.rs:337` and `migration.rs:392`.
- `USERPROFILE` is a constant four times (`mod.rs:83`, `command_path.rs:47`,
  `install/user_path.rs:28`, and `crates/orchestrator/src/workbench/parts.rs:38` outside install)
  and raw at `native_host.rs:326` and `migration.rs:375`.
- `PATH` is `PATH_VARIABLE` (`icons.rs:84`), `SEARCH_PATH_VARIABLE` (`command_path.rs:41`), and
  raw at `mod.rs:771` and `browser_package.rs:64`.
- The fallbacks `AppData/Roaming`, `AppData/Local`, `.config` and `.local/share` are spelled in
  seven files: `browser_package.rs:61`, `desktop_entry.rs:110`, `icons.rs:330` and `442`,
  `migration.rs:430` to `467`, `mod.rs:780` and `879` to `884`, `native_host.rs:416` and `417`,
  and `user_assets.rs:187`.
- `browser_package.rs:68` to `70` spell the Flatpak exports that `icons.rs:96`, `99` and `105` name
  as `FLATPAK_SYSTEM_EXPORTS`, `FLATPAK_USER_EXPORTS` and `APPLICATIONS_DIRECTORY`.

The rules differ:

- `native_host.rs:326` reads `USERPROFILE` before `HOME` on every system; `mod.rs:749` and
  `icons.rs:313` read it only on Windows.
- `migration.rs:375` to `396` takes `USERPROFILE` with no `HOME` fallback on Windows and drops
  relative values; the harness and native-host resolvers accept relative and empty values.
- `icons.rs:441` treats an empty `XDG_DATA_HOME` as unset; `desktop_entry.rs:108`,
  `user_assets.rs:185` and `browser_package.rs:58` do not. `workbench/parts.rs:959` treats an
  empty home as none; the install resolvers do not.
- `icons.rs:81` spells `ProgramFiles`; `native_host.rs:543` spells `PROGRAMFILES` and also reads
  `PROGRAMFILES(X86)`.

Proposed change: one `install/user_dirs.rs`: a `UserDirs` value (home, roaming, local, config,
data, state, program files and the `PATH` entries) resolved once from the environment under one
stated rule, a `UserDirs::within(root)` constructor for tests, and the variable names as its
constants. Each context takes a `&UserDirs`.

Ratings: payoff high, risk medium (the absolute-path and empty-value rule must be chosen on
purpose; taking the migration's rule everywhere changes behavior in unusual environments), effort
M. Installer output: no golden change expected, because the writer golden builds its context by
hand.

Verdict: partial. The nine resolvers and every count hold. Corrections: `ProgramFiles` against
`PROGRAMFILES` is no rule difference, since Windows reads variable names without regard to case;
the difference is that `native_host.rs:541-552` also reads `PROGRAMFILES(X86)` and drops empty
values. And one listed difference is a defect today, not a rule to choose: `mod.rs:758-764` passes
`XDG_CONFIG_HOME`, `APPDATA` and `CODEX_HOME` through unfiltered, and `harness_roots`
(`mod.rs:876-890`) takes an empty value as the root. With `XDG_CONFIG_HOME=` set, Zed's file is
`zed/settings.json` below the working directory; with `CODEX_HOME=` set, Codex's is `config.toml`
there. A detected client is then written in place (`record.rs:354` creates the parent).
`native_host.rs:406-420`, `desktop_entry.rs:108`, `user_assets.rs:185` and
`browser_package.rs:59` do the same, while `claude_config_dir` (`mod.rs:836`), `icons.rs:441` and
`workbench/parts.rs:959` already treat an empty value as unset, as the XDG base-directory rules
ask. The one stated rule already exists for the Ghostlight home, in locked source:
`crates/relay/src/home.rs:55`, `59`, `66` and `165` (USERPROFILE on Windows, HOME elsewhere,
absolute only), and `LegacyRoots` copies it. `UserDirs` should take that rule, and `LegacyRoots`
should keep its 1.x-only `real_root` (`migration.rs:419`).

Class: refactor, defect.

arch-1: none.

## INST-5: `config` and `roaming` always hold the same directory

Kind: an indirection that earns nothing.

Where: `HarnessContext.config` and `.roaming` (`crates/orchestrator/src/install/mod.rs:693` and
`694`), `HarnessRoots` (`mod.rs:863`), `harness_roots` (`mod.rs:869` to `893`), the Zed row
(`mod.rs:1265` to `1271`), and the test
`platform_roots_honor_effective_linux_and_windows_configuration` (`mod.rs:3416` to `3446`).

Problem: `harness_roots` gives both fields `%APPDATA%` (else `AppData/Roaming`) on Windows, and on
Linux sets `roaming` to a clone of `config` (`$XDG_CONFIG_HOME`, else `.config`). Every constructor
sets them equal (`HarnessContext::system`, `isolated_context` at `mod.rs:400`, and
`install/writer_golden.rs:418` to `421`), and the test asserts that they are equal on both
systems. The roster rows pick one or the other arbitrarily: `context.roaming` for Claude Desktop,
Visual Studio Code and the three Cline editor hosts, and `context.config` for Devin, Kilo Code and
goose. The Zed row branches on `context.windows` between the two; only its folder's case (`Zed`,
`zed`) needs the branch. The private view (`mod.rs:787`) and `packages()` (`mod.rs:816`) read
`roaming`.

Proposed change: one field, `config_root`, documented as `%APPDATA%` on Windows and
`$XDG_CONFIG_HOME` (else `~/.config`) on Linux. Drop `HarnessRoots.roaming`, and let the private
view and the Store package roots read the same field.

Ratings: payoff medium, risk low, effort S. Installer output: none.

Verdict: confirmed. The two fields are equal by construction in `harness_roots`
(`mod.rs:876-887`), and every context, the guide's (`agent_list.rs:115-119`) included, takes them
from it or sets them alike. The pick is not quite arbitrary: `roaming` mostly marks an editor or
desktop app's data root and `config` the XDG-style one, and the two would part only on macOS,
which no row supports. One field loses nothing and leaves the guide's table as it is.

Class: refactor.

arch-1: none.

## INST-6: The dialect dispatch is repeated in six functions

Kind: structure, duplicated logic, an indirection that earns nothing.

Where: `registration_state_at` (`crates/orchestrator/src/install/mod.rs:2096`), `apply_install`
(`mod.rs:2281`), `uninstall_at` (`mod.rs:2371`), `apply_fix` (`mod.rs:2388`), `manual_setup`
(`mod.rs:2858`) and `validate_located_config` (`mod.rs:3090`); the wrappers `edit_json`
(`mod.rs:2451`), `fix_json` (`mod.rs:2469`), `edit_toml` (`mod.rs:2539`) and `fix_toml`
(`mod.rs:2551`); the filters in `setup_detected` (`mod.rs:259`) and
`remove_stray_temporaries_at` (`mod.rs:326`).

Problem:

- Each of the six functions matches all four `ConfigDialect` arms.
- OpenCode's read-then-detect step (`read_or_empty`, then `opencode_dialect`) is written in
  `apply_install`, `uninstall_at`, `apply_fix` and `manual_setup` (`mod.rs:2295`, `2382`, `2406`
  and `2877`), and `registration_state_at` detects again from the source it read (`mod.rs:2113`).
- `edit_json` and `edit_toml` take `install: bool` only to rebuild a `RegistrationEdit`, and
  `fix_json` and `fix_toml` are one-line wrappers around the same `_with` functions.
- Two `Manual` arms can never run, because their functions return early for `Manual`
  (`mod.rs:2107` and `3098`).
- The eligibility filter "not manual, and not private to an app container" is written twice
  (`mod.rs:269` and `270`, `332` and `333`).

Proposed change: resolve a harness's format once (`Json(JsonDialect)`, `Codex` or
`Manual(ManualDialect)`, with OpenCode resolved from its file) and route install, fix, uninstall,
inspection and manual setup through one `edit(context, path, format, RegistrationEdit)`. Delete
the four wrappers and the `bool`, and add one `is_writable_from(context)` on the resolved harness.

Ratings: payoff medium-high, risk low (the writer golden proves the bytes), effort M. Installer
output: none; the writer golden is the check.

Verdict: confirmed. The six four-arm matches, the four OpenCode read-then-detect sites, the dead
`Manual` arms (early returns at `mod.rs:2101` and `3091`), the `bool` wrappers and the doubled
filter are as stated. Two notes on the change. Inspection, manual setup and locate validation are
not edits, so the shared piece is one resolved format with inspect, edit and manual-setup
methods, not one `edit` call. And OpenCode's dialect must stay resolved from each file at the
act, since Uninstall also edits other copies, each its own file (`mod.rs:2328-2330`). The writer
golden records inspection, locate, manual setup and every act for every dialect on three
computers, OpenCode's `mcp.servers` form included, so it checks the bytes.

Class: refactor.

arch-1: none; I3 and I4 changed these functions, not their dispatch.

## INST-7: Dialect keys are spelled out in each function

Kind: magic literals.

Where: `json_entry` (`crates/orchestrator/src/install/mod.rs:2620`), `json_collection`
(`mod.rs:2723`), `manual_json_setup` (`mod.rs:2882`) and `opencode_dialect` (`mod.rs:3003`); for
Codex, `inspect_toml` (`mod.rs:2160`), `toml_entry_state` (`mod.rs:2176`),
`missing_codex_desktop_environment` (`mod.rs:2205`), `forward_codex_desktop_environment`
(`mod.rs:2265`), `edit_toml_with` (`mod.rs:2557`), `manual_setup` (`mod.rs:2858`) and
`toml_entry_args` (`mod.rs:2913`).

Problem: the path of each dialect's server collection (`mcpServers`; `servers`;
`context_servers`; `mcp`; `mcp` then `servers`) is written out separately in `json_entry`,
`json_collection` and `manual_json_setup`, and again in `opencode_dialect`. The Codex code spells
`"mcp_servers"` nine times (`mod.rs:2163`, `2566`, `2603` to `2613`, `2869` and `2870`) and
`"env_vars"` four times (`mod.rs:2213`, `2268` to `2271`). It spells raw `"command"` and `"args"`
(`mod.rs:2181`, `2571`, `2600`, `2601`, `2865`, `2866` and `2915`) beside the existing
`COMMAND_KEY` and `ARGS_KEY` (`mod.rs:60` and `62`), and `"env"`, `"name"`, `"source"`, `"local"`
and `"remote"` are raw in the environment-forwarding check.

Proposed change: one `const fn collection_path(dialect) -> &'static [&'static str]` that the four
functions read, and named keys for the Codex table (the servers table, command, args, env,
env_vars and the forwarding members) in the Codex module.

Ratings: payoff medium, risk low, effort S. Installer output: none.

Verdict: confirmed. Nine `"mcp_servers"`, four `"env_vars"` and the raw `"command"` and `"args"`
at the seven cited lines are exact, and each JSON collection path is spelled in `json_entry`,
`json_collection` and `manual_json_setup`. `opencode_dialect` reads `mcp` and `mcp.servers` to
tell OpenCode's two forms apart, so it shares the keys, not a path. The keys are written into the
client's own file, so their bytes must not move; the writer golden pins them.

Class: refactor, persisted.

arch-1: none; I3 named only the JSON entry's keys (`mod.rs:60-64`).

## INST-8: `inspect` reads one registration state six times

Kind: structure, long function.

Where: `inspect` (`crates/orchestrator/src/install/mod.rs:1794` to `1895`): `retired`
(`mod.rs:1810`), `state` (`mod.rs:1811`), `detail` (`mod.rs:1822`), `evidence` (`mod.rs:1831`),
`can_fix` (`mod.rs:1847`) and `attention` (`mod.rs:1854`). The owned set
`Current | Updatable | Retired` in `toml_entry_state` (`mod.rs:2189`) and `holds_owned_entry`
(`mod.rs:2365`).

Problem: state, detail, evidence, fix eligibility and the attention cause are each derived by a
separate `match` over the same `registration`, so a new `RegistrationState` variant needs six
coordinated edits. The owned set is restated by hand in two places.

Proposed change: one match per `RegistrationState` variant that yields the state, the attention
cause and the evidence together, and a `RegistrationState::is_owned()` for the owned set.

Ratings: payoff medium, risk low, effort S. Installer output: none; the writer golden pins every
detail and evidence line.

Verdict: confirmed. Six readings of `registration` (`mod.rs:1810`, `1811`, `1822`, `1831`,
`1849`, `1854`) and the owned set at `mod.rs:2187-2190` and `2363-2366`. The hazard is sharper
than six coordinated edits: only the `state` match (`mod.rs:1811-1819`) is exhaustive. `detail`,
`evidence` and `attention` end in wildcard arms and the other two are `matches!`, so a new variant
compiles after one edit and silently takes the defaults elsewhere. A combined match must keep the
private view's precedence in `detail` (`mod.rs:1823`).

Class: refactor.

arch-1: I4 for `is_owned()`: ADR-0203 Decision 10.2 names one owned-entry predicate, which
`toml_entry_state` still restates. The six readings: none.

## INST-9: Words are still composed in install code

Kind: a vocabulary that leaves its module (the follow-up slice I10 named).

Where:

- `crates/orchestrator/src/install/native_host.rs`: `state_detail` (`native_host.rs:1274`) and
  `owned_elsewhere_detail` (`native_host.rs:1292`, sentences at `1304` and `1306`); nine inline
  `NativeHostError` texts (`native_host.rs:1420` to `1456`).
- `install/command_path.rs`: `not_applicable_detail` and `detail_for` (`command_path.rs:558` to
  `585`), and the `CommandPathError` texts (`command_path.rs:591` and `599`).
- `install/user_assets.rs`: `detail_for` (`user_assets.rs:296`) and its error texts
  (`user_assets.rs:337` and `345`).
- `install/browser_package.rs`: the package details (`browser_package.rs:120` to `136`).
- `install/desktop_entry.rs`: the error texts (`desktop_entry.rs:317` to `341`).
- `install/entry.rs`: the refusal at `entry.rs:72`.
- `install/reconcile.rs`: the report prefixes `"Browser connection: "` (`reconcile.rs:306` and
  `339`), `"{name} browser connection"` (`reconcile.rs:336`), `"MCP clients: "`
  (`reconcile.rs:350`) and `"{} MCP client"` (`reconcile.rs:358`), and
  `WINDOWS_ENTRIES_FAILURE = "Start menu"` (`reconcile.rs:285`).
- `install/mod.rs:239`: the Check message `"{} was checked without changing configuration."`.
  I10's list of what moved names the "apply and locate messages"
  (`docs/tasks/arch-1/LEDGER-install.md:252`), and this apply message stayed in place.

Problem: the module doc of `crates/orchestrator/src/language/deployment.rs` says the install code
"neither words anything itself" (ADR-0199, the development channel amendment; ADR-0203 Decision
8). The I10 ledger note lists the rest as what stayed and says "A later slice can move them the
same way" (`LEDGER-install.md:260` to `263`).

Proposed change: move them verbatim into `language/deployment.rs`; each module keeps its typed
state and passes the facts.

Ratings: payoff medium, risk low, effort S to M. Installer output: byte-identical if moved
verbatim; the writer golden pins the Check line. This moves words only. It does not merge the five
registries' state enums, which ADR-0203 considered and did not take (G3-05).

Verdict: confirmed. Every cited text is in place, the nine `NativeHostError` texts included, and
`mod.rs:238-241` is an apply message I10 left although its ledger says apply messages moved.
Until this lands, `language/deployment.rs:6-7` ("neither words anything itself") is false. Some
of these words reach the agent, not only a person: `browser_package::detail` rides
`BrowserRegistration.package_detail` (`native_host.rs:778`) into recovery's `details`
(`browser/recovery.rs:215` and `992`), a native-host repair error is formatted there too
(`browser/recovery.rs:291-295`), and both become the `details` fact of a browser-recovery refusal
(`work/mod.rs:2660-2672`). Moved verbatim, nothing anyone reads changes.

Class: refactor, model-visible.

arch-1: I10 (Decision 8), whose ledger leaves these for "a later slice"
(`LEDGER-install.md:260-263`); it does not reopen G3-05.

## INST-10: `deployment::Outcome` shares its name with the language `Outcome`

Kind: naming.

Where: `pub struct Outcome` (`crates/orchestrator/src/install/deployment.rs:626`), imported bare
by `crates/orchestrator/src/cli/deployment.rs:8`; `pub enum Outcome`
(`crates/orchestrator/src/language/outcome.rs:406`); a private `enum Outcome` in
`install/icons.rs:537`; and, outside this area, `pub enum Outcome` in
`crates/orchestrator/src/governance/migrate.rs:50`.

Problem: AGENTS.md makes the typed `Outcome` in `language/outcome.rs` an invariant: a completed
action's sentence, its safe next steps and its measurements "come from one typed `Outcome`". A
second public `Outcome` for an install's result misleads anyone who searches for it.

Proposed change: rename it `InstallOutcome`, to pair with `InstallPlan`. Give the private one in
`icons.rs` ("What one look decided for one agent") a local name such as `LookResult`.

Ratings: payoff medium, risk low, effort S. Installer output: none.

Verdict: confirmed. `pub struct Outcome` at `install/deployment.rs:626`, imported bare at
`cli/deployment.rs:8`, beside `language/outcome.rs:406`, the private `icons.rs:537` and
`governance/migrate.rs:50`; `InstallPlan` is there to pair with.

Class: refactor.

arch-1: none.

## INST-11: Two engines stop processes

Kind: duplicated logic, a concern in the wrong module.

Where:

- `crates/orchestrator/src/install/migration.rs`: `stop_legacy_images` (`migration.rs:1513`),
  `StopOutcome` (`migration.rs:1557`), `still_running` (`migration.rs:1579`), `any_running_from`
  (`migration.rs:1592`), `process_table` (`migration.rs:1600`) and `running_from`
  (`migration.rs:1613`), with `STOP_WAIT` 10 s (`migration.rs:75`) and an unnamed 25 ms sleep
  (`migration.rs:1549`).
- `install/deployment.rs`: `STRAY_STOP_WAIT` 5 s (`deployment.rs:344`), `REMOVED_IMAGE_MARKER`
  (`deployment.rs:347`), `running_image` (`deployment.rs:351`), `stray_processes`
  (`deployment.rs:363`), `stray_refresh` (`deployment.rs:389`), `classify` (`deployment.rs:401`),
  `same_file_name` (`deployment.rs:436`), `lies_under` (`deployment.rs:445`), `stop_found_strays`
  (`deployment.rs:470`) and `stop_strays` (`deployment.rs:492`), with `POLL` 25 ms
  (`deployment.rs:73`).
- `SystemProcessControl::stop_image`, `is_running` and `wait_ready` call into the migration
  (`deployment.rs:259`, `263` and `293`). The `ProcessControl` docs restate `QUIT_WAIT` and
  `READY_WAIT` as "10 s" and "30 s" (`deployment.rs:173` and `189`).

Problem: both engines read the sysinfo process table, kill, and poll for exit with a zombie check,
each with its own deadline and sleep. They match images differently: the migration canonicalizes
both the process's executable and the image (`running_from`), which cannot match a process whose
file was deleted, while the deployment strips Linux's `" (deleted)"` marker (`running_image`).
`wait_ready` asks the one-time 2.0 migration module whether the current service still runs.
`same_file_name` and `lies_under` restate parts of the `entry.rs` path rules.

Proposed change: one `install/processes.rs` with a process-table read, one image match (with the
deleted marker) and one kill-and-wait with a named poll interval, used by the migration, the stray
sweep and `SystemProcessControl`. The trait docs link the wait constants instead of restating
their values.

Ratings: payoff medium-high, risk medium (process behavior), effort M. Installer output: no golden
change; run the process journeys after.

Verdict: partial. Every cited item is there, and the trait docs restate `QUIT_WAIT` and
`READY_WAIT` (`deployment.rs:80` and `83`). The change as written merges two contracts. The
migration's stop must succeed or the install fails, and it refuses the running installer
(`migration.rs:1519-1547`); the stray stop reports what would not stop and blocks nothing
(`deployment.rs:529-545`, `974`). The waits differ too: the stray wait checks that each pid still
runs the same image (`deployment.rs:511-520`), and the migration's does not (`migration.rs:1539`).
The matches answer two questions: an exact list of existing images, against anything under the
home by place and name (`deployment.rs:401`). No caller gives the migration engine an image that
is gone (the 1.x images before `finish`, the files `running_images` lists, the new service in
`wait_ready`), so the canonicalize gap is not a live defect, and giving it the deleted-marker
match changes what `quit` and `wait_ready` see (`deployment.rs:248` and `293`). The shared table
read and one kill-and-wait that takes a deadline and a fail-or-report choice are a refactor; the
one image match is a behavior change for the process journeys.

Class: refactor.

arch-1: none.

## INST-12: Roster boilerplate and a field-by-field copy

Kind: duplicated literals, an indirection that earns nothing.

Where: `AgentRow` (`crates/orchestrator/src/install/mod.rs:988`), `ROSTER` (`mod.rs:1120` to
`1574`), `AgentRow::on` (`mod.rs:1631` to `1654`) and `HarnessDefinition` (`mod.rs:899`).

Problem: of the 23 rows, 20 spell an empty `aliases`, 21 an empty `candidates`, 22
`own_package: None`, 22 `icon_family: None`, 19 an empty `runs_inside` and 15 an empty `icons`,
and 18 rows have the same value for `id`, `agent_id` and `product_id`. The Cline extension folder
`saoudrizwan.claude-dev` appears four times (`mod.rs:1386`, `1391`, `1414` and `1437`), with
`User/globalStorage` three times, and the Cline installation page URL twice. `AgentRow::on` copies
11 row fields one by one into the 17-field `HarnessDefinition` and derives `label` from `name`.

Proposed change: a `const BASE: AgentRow` used with `..BASE` struct update (or a `const fn`
constructor), so each row states only what is special; a `CLINE_EXTENSION` constant and a
`cline_settings(context, editor)` helper; and a `HarnessDefinition` that holds
`row: &'static AgentRow` beside its resolved fields (path, default path, label, located
executable, staleness and copies).

Ratings: payoff medium, risk low, effort S to M. Installer output: none. This keeps one table;
splitting it into an agent table and host rows is deferred to 2.x by ADR-0203 (G3-10).

Verdict: confirmed. 23 rows; the counts (20, 21, 22, 22, 19, 15 and 18), the four spellings of
`saoudrizwan.claude-dev`, the three of `User/globalStorage`, the two Cline URLs and the 11 of 17
copied fields are exact. On the form: `Monogram::packaged` is a `const fn` (`agent/image.rs:62`),
so a `const BASE` compiles, but it must hold placeholders for what every row sets (id, agent id,
product id, name, target, monogram, path, dialect), and a row that forgot one would still compile.
The `const fn` constructor that takes those keeps the compiler's check. Only `AgentRow::on` builds
a `HarnessDefinition` (`mod.rs:1636`), so holding the row is a local change.

Class: refactor.

arch-1: none; it keeps G3-10's one table, so it reopens nothing.

## INST-13: The per-agent catalog loop is written twice, and `IconSource` is misnamed

Kind: duplicated logic, naming.

Where: `icon_catalog` (`crates/orchestrator/src/install/mod.rs:1750`) and `app_catalog`
(`mod.rs:1774`); `AgentIconSources` (`install/icons.rs:244`, `sources` at `icons.rs:250`) and
`AgentAppSources` (`install/apps.rs:114`, `sources` at `apps.rs:118`, `commands` at `apps.rs:120`);
`IconSource` (`icons.rs:207`); the row fields `icons` and `executables`.

Problem: both functions group roster rows by `agent_id` with the same find-or-push loop. The
module doc of `apps.rs` defines where an agent's app is installed as the `IconSource`s its icon is
read from, so the row's `icons` field is really the app's install places, and the row's
`executables` surface as `commands`.

Proposed change: one `agent_apps()` that groups once and returns each agent's id, name, places,
commands and icon family. Rename `IconSource` to `AppPlace` and the row's `icons` to `app_places`,
keeping the serialized words of `IconSourceKind` (`icons.rs:233`), which icon manifests and
`doctor --json` carry.

Ratings: payoff medium-low, risk low, effort S. Installer output: none if the serialized words
stay.

Verdict: confirmed. `icon_catalog` and `app_catalog` run the same find-or-push loop, and
`apps.rs:4` defines an app's places as the `IconSource`s. The words to keep are those of
`IconSourceKind` (`icons.rs:233`), which a cached icon's manifest and `doctor --json` carry;
`IconSource` and the row field are never serialized.

Class: refactor.

arch-1: none.

## INST-14: Platform helpers are scattered or live in the wrong module

Kind: magic literals, confusing namespaces.

Where and problem:

- The Windows program-name rule lives in five places: `executable_name(name, windows)`
  (`crates/orchestrator/src/install/mod.rs:3195`), `executable_name(name, platform)`
  (`install/native_host.rs:1351`), `EXE_SUFFIX` in `install/apps.rs:79` and
  `install/icons.rs:132`, and `install/layout.rs`, which uses the host's `EXE_SUFFIX`
  (`layout.rs:11` and `96`) and hard-codes `ghostlight-desktop.exe` (`layout.rs:23`).
  `is_executable_file` checks the `exe` extension again (`mod.rs:3110`).
- In `native_host.rs`, `format!("{HOST_NAME}.json")` is written five times (`native_host.rs:1207`,
  `1243`, `1312`, `1331` and `1337`). The registry key shape
  `Software\<vendor>\NativeMessagingHosts\<host>` is written four times (`native_host.rs:392`,
  `1255`, `1257` and `1346`), two of them with the isolated prefix `Software\Ghostlight\Isolated`.
  `NativeMessagingHosts` is raw nine times (`native_host.rs:392`, `462`, `476`, `490`, `504`,
  `1255`, `1257`, `1321` and `1346`) beside `LINUX_NATIVE_MESSAGING_COMPONENT`
  (`native_host.rs:516`). `BROWSERS` stores `windows_directory` as `windows_vendor` plus
  `Application` (`native_host.rs:460`, `474`, `488` and `502`), and `linux_directory` with a
  `/NativeMessagingHosts` suffix (`native_host.rs:462`, `476`, `490` and `504`) that
  `user_data_directory` (`native_host.rs:521`) strips back off.
- Ghostlight's one atomic replace and private creator (`Replace`, `atomic_replace`,
  `atomic_replace_keeping_access`, `is_temporary` and `create_private`, `install/record.rs:318` to
  `490`) live in the module documented as "The install record", and `governance`, `service` and
  `cli` import them from there. The same-location rule (`same_path`, `spelled_alike`,
  `same_location`, `same_location_on` and `lies_within`, `install/entry.rs:136` to `184`) lives in
  "The entry rule" module. Slice I2 placed it there on purpose
  (`docs/tasks/arch-1/LEDGER-install.md:165`), so this only renames its home. `icons.rs:44`
  imports `lock`, `HOME_VARIABLE` and `USER_PROFILE_VARIABLE` from the harness root.

Proposed change: one `program_file_name(name, windows)` and a `WINDOWS_PROGRAM_SUFFIX` constant in
`layout.rs`; a manifest file-name constant and one registry-key builder in `native_host.rs`;
`BROWSERS` rows that hold a `linux_config_directory` and derive both paths; and `install/files.rs`
(atomic replace, private create) and `install/paths.rs` (the same-location rule) as their own
modules.

Ratings: payoff medium-low, risk low, effort S. Installer output: none.

Verdict: confirmed. The `.exe` spellings, the five manifest names, the four key shapes, the nine
raw `NativeMessagingHosts` and the `BROWSERS` round trip are exact. Notes on the change: the
`.exe` rule is two facts, a Windows flag that lets tests model a Windows computer on Linux
(`mod.rs:3195`, `native_host.rs:1351`) and this build's suffix, which `layout.rs` spells as a
`cfg` constant pair, as the locked relay spells the edge and the service. `record.rs:8-11` does
document the replace and the creator, and ADR-0203 Decisions 11.1 and 11.2 name
`install::record`'s entry points and promise that no caller outside install changes, so
`install/files.rs` needs `record` re-exports or edits to `governance/managed/cli.rs:12`,
`governance/managed/mod.rs:25`, `governance/migrate.rs:17`, `governance/mod.rs:29`,
`service/hint.rs:20` and `cli/desktop.rs:116`. The manifest name and the registry keys are what
the browsers read; tests pin them (`native_host.rs:1761-1859`).

Class: refactor, persisted.

arch-1: I2 (Decision 11.2), which placed both rules; this renames their homes only.

## INST-15: Dead and unread items

Kind: dead code, unread fields.

Where: `HarnessRegistry::resymbol` (`crates/orchestrator/src/install/mod.rs:196`);
`HarnessSummary.product_id` (`mod.rs:492`) and `AgentRow.product_id`; `HarnessSummary.target`
(`mod.rs:499`); `HarnessSummary.name`.

Problem:

- `resymbol` has no caller, not even a test. The I6 ledger note says surface slice S3 will make
  the window's `icons_changed` call it (`docs/tasks/arch-1/LEDGER-install.md:195` to `198`); S3 is
  still `todo` (`docs/tasks/arch-1/LEDGER-surface.md:22`).
- `HarnessSummary.product_id` is read by no production code, only by the writer golden's
  `product:` line (`install/writer_golden.rs:829`) and the workbench wire
  (`harnesses[].product_id`, `crates/orchestrator/src/workbench/parts/tests.rs:569`); the window's
  scripts never read it. `AgentRow.product_id` is read only to copy it there.
- `HarnessSummary.target` is likewise read only by the writer golden (`writer_golden.rs:832`) and
  the wire. The row's `target` is used: it builds `label` (`mod.rs:1691`) and the guide's Target
  column (`install/agent_list.rs:286`).
- `HarnessSummary.name` repeats `agent.name`: `inspect` builds `agent` from the same name
  (`mod.rs:1867`), and `crates/orchestrator/src/workbench/setup.rs:488` reads the copy.

Proposed change: wire `resymbol` in S3 or delete it. Dropping `product_id` needs the ADR-0125
Decision 1 and ADR-0148 amendments that ADR-0203 deferred to 2.x (G3-10). Whether the summary's
`target` and `name` stay on the wire is the surface lane's call.

Ratings: payoff low-medium, risk low, effort S. Installer output: dropping a field changes the
writer golden and the workbench wire.

Verdict: partial. `resymbol` has no caller and no test, and `product_id` and `target` reach only
the writer golden and the wire. Corrections: `HarnessSummary.name` is read, so it is a duplicate,
not unread: the window takes it through `agentOf`'s spread (`ui/lib/view.js:1204`, read at
`1265`, `1324`, `1440`, `1449` and `1452`), and `workbench/setup.rs:488` reads it. The summary
also goes out whole in `doctor --json` as `mcp_clients` (`cli/doctor.rs:50`, `130` and `293`),
so dropping a field changes that output as well. And deleting `resymbol` would undo I6's
deliverable for S3, whose Changes say "`icons_changed` re-symbols the cached roster" and which is
still `todo`; the consistent end is to wire it there, with the test I6 did not add.

Class: refactor.

arch-1: I6 with S3 for `resymbol`; dropping `product_id` would reopen G3-10 (Considered and not
taken; ADR-0203 keeps ADR-0125 and ADR-0148).

## Overall

The code is careful and documented item by item, and most literals are already named constants
beside their use. The cost is placement. The install root is also the harness feature, the agent
catalog, and the icon and app identity that `crate::agent`, provenance and the workbench depend
on, and each subsystem works out home directories, process control and file names on its own. The
one change that would help most is INST-2 together with INST-3: move the harness feature into
`install/harness/` under one glossary, so `install/mod.rs` becomes an index and the other findings
land in small, obvious files.

## Skeptic summary

Verdicts: 9 confirmed (INST-5, 6, 7, 8, 9, 10, 12, 13 and 14), 6 partial (INST-1, 2, 3, 4, 11 and
15), none refuted. The partials narrow a scope or correct a proposal, and one of them uncovers a
live defect (INST-4). No proposed change needs a writer or native golden to change, apart from
INST-3's `target` option and INST-15's deferred drops, as long as INST-2 leaves `writer_golden`
and `agent_list` where the goldens' headers say they are.

What I would do first:

1. INST-4, its empty-value half. It is the one defect here: an empty `XDG_CONFIG_HOME`, `APPDATA`
   or `CODEX_HOME` makes an install write a client's entry below the working directory, and for
   Codex into a project's own `config.toml`. One helper that drops empty and relative values, with
   a test per resolver, fixes it ahead of the larger `UserDirs` move. No golden changes, because
   the goldens build their contexts by hand.
2. INST-9. It is a verbatim move, so nothing anyone reads changes, and it makes the stated rule of
   `language/deployment.rs` true. Some of the words reach the agent through browser-recovery
   refusals, which is where a word outside `language/` costs most.
3. INST-8. It is small, the writer golden pins it line by line, and it finishes ADR-0203 Decision
   10.2's one owned-entry predicate, which `toml_entry_state` still restates. It also closes five
   matches that a new state would fall through without a compiler error.

INST-1 comes next, with the migration's 1.x names kept apart from the readers' 2.0 names. INST-2
pays most once the others have landed, and it costs most.

The INST-3 glossary. "Harness" is the right core word for one agent's configuration on one host.
The window's wire (`harnesses`, `harnesses_in_use`, `harness_act`, `{harness: id}`), the
persisted `harness-locations.json`, the `Harness*` types and `docs/1.0/ARCHITECTURE.md:777`
already use it, so it is the only choice that renames nothing on a wire or on disk; "host"
already means a host app and a native-messaging host. The glossary misses three uses. "Harness"
also names a live MCP connection: `harness_attached` and `harness_detached`
(`crates/bridge/src/diagnostics.rs:51-52`, written to diagnostics logs) and AGENTS.md's "harness
connections". "Client" is a machine word as well as a person's: `mcp_clients` in `doctor --json`
(`cli/doctor.rs:293`) and in the status answer (`docs/design/relay-protocol-4.md:135`), and
`mcp_client` in the `--plan` JSON (`install/migration.rs:483-490`). And `target`, the host's
label, has no entry. No proposed rename touches a wire or persisted word: `AgentRow`,
`HarnessDefinition`, `ClientIds`, `ClientName`, `Registrations`, `client_ids`, `reconcile_clients`
and the `runs_inside` parameter are never serialized and appear in no golden. A sweep under the
glossary must still leave the JSON words above alone, and also `ReconcileReport.clients_rewritten`,
`clients_unchanged` and `UnchangedEntry.client`, which derive `Serialize`
(`install/reconcile.rs:51-67`) though nothing prints them today. Renaming `target` is the one
option that changes bytes: the window wire, `doctor --json`, the writer golden's `target:` line
and the guide's column.
