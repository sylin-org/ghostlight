# T01: one loader for the policy file

## Goal

Implement ADR-0023: `parse_manifest` becomes the sole reader/parser/validator of the policy
file; the config layers derive from the already-parsed `Manifest.config`; `parse_org_config`
and `load_and_resolve` are deleted. This fixes the stage-3 outage: today ANY policy file at
the org path is a fatal server start (two parsers, mutually exclusive schema gates 2 vs 3).
After this task a schema-3 org policy with a config block boots the server, locks its
mandatory keys, and enforces its grants.

## Authority

ADR-0023 (`docs/adr/0023-one-loader-for-the-policy-file.md`) is normative for every semantic
here; where this prompt and the ADR disagree, THE ADR WINS (record the deviation in the
ledger). ADR-0022 still owns what the manifest SAYS; this task changes only how it is LOADED.

## Depends on

Nothing; first stage-4 task. Per BOOTSTRAP: work on branch `stage-4` (create from `stage-3`
if absent), confirm LEDGER.md RESUME HERE names t01 and the tree is clean.

STOP preconditions: `src/governance/config/load.rs` contains `parse_org_config` with a
`schema_num != 2` gate, and `src/governance/manifest/document.rs::parse_manifest` gates
`schema == 3`. If either is untrue the tree is not the one this prompt was written against;
STOP and record.

## Current behavior (verified 2026-07-03 against stage-3 head `b4b2faf`; re-read the tree)

- `src/governance/config/load.rs`: `parse_org_config(content, path, domain_pattern_valid)
  -> Result<OrgConfig>` gates `schema_num != 2` (line ~144), then hand-walks the untyped
  `config` array (unknown-member check, key/value/level validation via
  `key_def` + `layers::validate_value`, duplicate-key rejection, level match). `OrgConfig
  { mandatory, recommended }` (two `serde_json::Map`s). `read_layers` reads BOTH files
  (user via `parse_user_config`, org via `parse_org_config`). `load_and_resolve` (line
  ~299) is dead public API with zero callers.
- `src/governance/manifest/document.rs`: `parse_manifest` fully deserializes and validates
  the manifest including `config: Vec<ConfigEntry>` (`validate_config_entry`, line ~337:
  per-entry key/value validation; NO duplicate-key check across entries). `ConfigEntry
  { key, value, level: Level }`; `Level { Mandatory, Recommended }`.
- `src/governance/manifest/source.rs`: `load_policy(user_source, domain_pattern_valid) ->
  Result<LoadedPolicy, LoadError>`; `LoadedPolicy { manifest: Option<Manifest>, origin:
  Option<ManifestOrigin>, user_manifest_ignored: bool }`. `manifest_config_as_user_layer`
  (line ~232) returns the flat user-layer map for a USER-sourced manifest (mandatory
  entries downgraded with a warn) and an EMPTY map for an org-sourced manifest, explicitly
  because `parse_org_config` "already reads the same file independently".
- `src/governance/config/reload.rs`: `ConfigStore::load_initial(domain_pattern_valid)` and
  `load_initial_with_manifest_config(domain_pattern_valid, manifest_user_config: Map)`
  (line ~139) read org (via `read_and_parse_org` -> `parse_org_config`, line ~448) and
  user files, `compose_initial` (fail-loud), merge the manifest user map into
  `last_good.user`, resolve, and build the store. `reresolve` (line ~181) re-reads both
  files the same way; `plan_reload` applies keep-last-good (org ERROR, user WARN).
  `WatchSources { user_config, org_policy, manifest: None }` (line ~83).
- Consumers of the double parse: `src/transport/mcp/server.rs` (~line 61) calls
  `load_initial_with_manifest_config(pattern::is_valid_pattern,
  manifest_config_as_user_layer(&loaded_policy))`; `src/doctor.rs::governance_section_lines`
  (~line 183) does `load_policy` then the same store call; `src/governance/config/cli.rs::
  resolve_with_warnings` (~line 78) uses `load::read_layers`, and `shadow_line` (~line 131)
  calls `load_policy` AGAIN in the same invocation;
  `src/governance/config/presets.rs::resolve_current_and_candidate` (~line 132) uses
  `read_layers`.
- `src/main.rs::run_server` (~line 442) performs the one startup `load_policy` and passes
  `LoadedPolicy` to `mcp::server::run`.

## Required behavior

### 1. Duplicate-config-key validation moves into the manifest (`document.rs`)

In `parse_manifest`'s config validation pass, reject a manifest whose `config` array
contains the same `key` twice. Use the existing `field_error` shape; for a duplicate at
index `i`, the path is `config[{i}].key` and the message is exactly:

    duplicate config key '{key}'

This applies to every manifest origin (deliberate tightening, ADR-0023 Decision 3).

### 2. The pure org split (`config/load.rs`)

Add (placement: with `OrgConfig`, replacing `parse_org_config`):

    /// Split already-validated manifest config entries into the org layers (ADR-0023
    /// Decision 2). Pure; duplicates are impossible here because parse_manifest rejects
    /// them (ADR-0023 Decision 3).
    pub fn org_config_from_entries(entries: &[ConfigEntry]) -> OrgConfig

`Level::Mandatory` entries populate `mandatory`, `Level::Recommended` populate
`recommended` (key -> value, cloned). Import `ConfigEntry`/`Level` from
`crate::governance::manifest::document` (an intra-governance edge; the arch test only
forbids browser/transport/mcp/native/url).

DELETE `parse_org_config` and its test module entries (`org_file_violations_are_errors`
entirely; rework `org_entries_populate_layers_by_level` to build its `OrgConfig` via
`org_config_from_entries` from typed `ConfigEntry` values, keeping its layer-resolution
assertions byte-identical). DELETE `load_and_resolve` (dead; zero callers -- verify with
`rg -n "load_and_resolve" src/ tests/` first; if a caller appeared, STOP and record).

### 3. The store consumes the parsed policy (`reload.rs`)

- `load_initial_with_manifest_config` is RENAMED `load_initial_with_policy` and reshaped:

      pub fn load_initial_with_policy(
          domain_pattern_valid: fn(&str) -> bool,
          loaded_policy: &crate::governance::manifest::source::LoadedPolicy,
      ) -> crate::Result<Arc<ConfigStore>>

  It derives the org layers from the policy instead of re-reading the org file: when
  `loaded_policy.origin == Some(ManifestOrigin::OrgPolicyFile)`, `last_good.org =
  org_config_from_entries(&manifest.config)`; otherwise `OrgConfig::default()`. The user
  config file read (`read_and_parse_user`), `compose_initial`'s fail-loud posture for the
  USER file, the `manifest_config_as_user_layer` merge into `last_good.user` (now computed
  INSIDE this function from `loaded_policy` rather than passed in), warnings logging, and
  the store construction are otherwise unchanged. `load_initial` (the no-policy
  convenience) delegates with an all-open `LoadedPolicy` (or is reshaped equivalently;
  keep a zero-argument-beyond-checker form for existing callers -- find them with
  `rg -n "load_initial\b" src/ tests/`).
- `read_and_parse_org` re-points to the single loader: read the org path; `NotFound` ->
  `OrgConfig::default()`; otherwise `parse_manifest(&content, &path.display().to_string(),
  domain_pattern_valid)` and `org_config_from_entries(&manifest.config)`. Map a
  `ManifestError` via Display ALONE -- `crate::Error::Config(e.to_string())` -- because
  `parse_manifest`'s `source_label` already carries the path; do NOT prepend the path a
  second time (a `{path}: {error}` wrapper would double it). Consequence, intended and
  sanctioned (BOOTSTRAP rule 8): an org file whose GRANTS are invalid now also fails a
  config reload (keep-last-good + ERROR) -- it already failed startup fatally via
  `load_policy`.
- `plan_reload`, `compose_initial`, the watcher, fingerprints, and `WatchSources` are
  UNCHANGED (the `manifest: None` slot stays `None`; ADR-0025/t06 owns it).
- `reload.rs` inline tests are CONTENT-FREE (they inject typed `OrgConfig` values or
  `Err(String)` via `reload_with`/`plan_reload`; none fabricates raw org JSON) and need
  no fixture changes; only `read_and_parse_org`'s implementation changes. The only
  schema-2 org JSON fixtures live in `load.rs` and are owned by section 2. If you find a
  reload.rs test fabricating raw org JSON, the tree has drifted: rework it to a minimal
  schema-3 manifest and record the deviation.

### 4. Consumers collapse to one parse each

- `src/transport/mcp/server.rs` (~line 61): pass `&loaded_policy` to
  `load_initial_with_policy`; delete the `manifest_config_as_user_layer` call site (the
  store computes it now).
- `src/doctor.rs::governance_section_lines`: same swap; exactly one `load_policy` per run
  (already true) and zero direct org-file reads.
- `src/governance/config/cli.rs`: `resolve_with_warnings` gains the policy: it calls
  `source::load_policy(std::env::var("BROWSER_MCP_MANIFEST").ok().as_deref(), ...)` ONCE,
  derives layers via the reshaped `read_layers` (below), and RETURNS the `LoadedPolicy`
  alongside the resolution so `run_list` passes it to `shadow_line` instead of `shadow_line`
  re-loading it (delete `shadow_line`'s own `load_policy` call; its signature gains
  `loaded_policy: &LoadedPolicy`). A broken policy file now surfaces in `config list` as
  the same hard error the server gives (it is no longer swallowed by `.ok()?` for the
  layer read; keep `shadow_line` itself returning `None` when there is no manifest).
- `load::read_layers` is reshaped to take the already-loaded policy:
  `read_layers(domain_pattern_valid, loaded_policy: &LoadedPolicy) -> Result<LoadedLayers>`
  reading ONLY the user config file and deriving `org` via `org_config_from_entries` /
  the user-layer map via `manifest_config_as_user_layer`. `LoadedLayers` keeps its shape;
  the manifest user-layer map merges INTO `user.values` with the user config FILE winning
  on a key collision -- transcribe the exact precedence of
  `reload.rs::merge_manifest_user_config` (file entries inserted last), so CLI resolution
  and the server store can never disagree. Consequence, intended and sanctioned
  (BOOTSTRAP rule 8): `config list`/presets now see a user manifest's config entries
  (previously the CLI ignored them entirely).
- `src/governance/config/presets.rs::resolve_current_and_candidate`: same pattern (one
  `load_policy`, reshaped `read_layers`).
- `src/governance/manifest/source.rs::manifest_config_as_user_layer`: behavior is
  UNCHANGED (org-sourced -> empty map; user-sourced -> flat map with mandatory-downgrade
  warn), but its doc comment must be rewritten: the org branch is empty because org
  entries take the ORG channel (`org_config_from_entries`), not because a second parser
  reads the file. Its two inline tests
  (`manifest_config_as_user_layer_downgrades_mandatory_and_ignores_org_origin`,
  `manifest_config_as_user_layer_empty_when_no_manifest`) keep passing as-is.

### 5. Error-message ripple

`rg -n "parse_org_config|expected 2" src/ tests/` after the change must return no live
code. Tests that pinned `parse_org_config` error strings die with it (section 2). Startup
failure for a broken org file now always carries `parse_manifest`'s error taxonomy
(including the ADR-0022 schema-2 pointer message, which existing document.rs tests
already pin).

## Constraints

1. One task, one commit: `fix(architecture): t01 one loader for the policy file`.
2. Do NOT touch `src/transport/mcp/schemas/tools.json`, `tests/tool_schema_fidelity.rs`,
   `examples/`, templates, simulate/explain fixtures, or the extension. Guard-test
   expectations in `tests/all_open_golden.rs` / `tests/mcp_protocol.rs` unchanged (the
   no-policy path must remain a pure no-op).
3. `tests/architecture.rs` stays green (everything here is intra-governance or transport).
4. The user config file's lenient parser (`parse_user_config`) is untouched.
5. ASCII only; no new dependencies; delete what you replace (BOOTSTRAP rule 13).

## Tests (minimum)

1. `duplicate_config_key_is_a_field_error` (document.rs inline): a schema-3 manifest with
   two `audit.enabled` entries fails; the error string contains `config[1].key` and
   `duplicate config key 'audit.enabled'`.
2. `org_config_from_entries_splits_by_level` (load.rs inline): one Mandatory + one
   Recommended entry land in the right maps with the right values; empty input yields
   `OrgConfig::default()`.
3. Reworked `org_entries_populate_layers_by_level` (load.rs inline): same layer-resolution
   assertions as today, org side built via `org_config_from_entries`.
4. `org_sourced_policy_config_reaches_the_org_layers` (reload.rs inline or a new
   `#[cfg(test)]` helper path): build a `LoadedPolicy` whose manifest (org origin) carries
   `audit.enabled = true` at `level: mandatory`; `load_initial_with_policy` yields a store
   whose `current()` has audit enabled and the key locked at the org-mandatory source.
5. `org_policy_file_with_config_boots_the_server` (NEW integration test, `#[cfg(windows)]`,
   in `tests/manifest_validation.rs`): write a temp dir containing
   `browser-mcp/policy.json` with a schema-3 manifest carrying one read-only grant plus
   TWO mandatory config entries: `audit.enabled = true` and `audit.file.path` pointing at
   a unique temp path (so the spawned server never writes the machine-default
   audit.jsonl); spawn the binary (the `CARGO_BIN_EXE_browser-mcp` + unique
   `BROWSER_MCP_ENDPOINT` pattern used across the integration suites) with env
   `ProgramData` pointing at the temp dir; drive `initialize` + `tools/list`; assert the
   server answers (the outage regression: today this exits at startup) and the advertised
   list is the GOVERNED set for a read-only grant -- TRANSCRIBE the expected tool list
   from `tests/tool_advertisement.rs::
   read_only_manifest_advertises_everything_except_write_and_execute_tools`; do not
   re-derive it. Clean up temp files.
6. The full existing suite stays green, including every `reload.rs` keep-last-good test
   and `tests/policy_explain.rs` / `tests/policy_simulate.rs` / `tests/policy_init.rs`
   untouched.

## Verification

`cargo fmt` then `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
clean; `cargo test` fully green. `rg -n "parse_org_config|load_and_resolve" src/ tests/`
-> no live hits. `rg -n "expected 2" src/` -> no hits outside historical docs. Manual
smoke (run and then UNDO): copy `examples/research-read-only.json` to
`%ProgramData%\browser-mcp\policy.json`, run `cargo run -- doctor`, confirm the governance
section renders the manifest (no "config resolution is broken"); DELETE the file
afterwards and confirm doctor shows all-open again. ASCII scan on touched files. Append
ONE deferred live check to `docs/tasks/stage-2/BROWSER-TESTS.md`:

    ## t01-1: org-path policy file loads live (the stage-3 outage fix)
    Changed: t01 made parse_manifest the sole loader for the policy file; previously any
    org-path policy file was a fatal startup error.
    Steps: place a schema-3 manifest with a read-only grant and a mandatory audit.enabled
    config entry at the platform org policy path; restart the MCP client; run tabs_context,
    a navigate to a granted host, and a computer left_click.
    Expect: the server starts; the client's tool list is the governed (filtered) set; the
    navigate succeeds; the left_click is denied naming the capability; the audit file
    records the calls. Removing the file and restarting restores all-open.

Then update LEDGER.md (task-log entry + RESUME HERE -> t02) and commit.

## Out of scope

- Manifest hot-reload, the `WatchSources.manifest` slot, swappable Governance, session
  events (t06 / ADR-0025).
- The tool registry, pipeline, governance API, tab-URL, deletions of port seams (t02-t07).
- Any change to manifest CONTENT semantics (ADR-0022 owns schema 3), examples, templates.
- `parse_user_config`, explain.rs's deliberate parallel user-config parser, preset naming.
