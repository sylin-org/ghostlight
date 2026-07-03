# 0023. One loader for the policy file

- Status: Accepted
- Date: 2026-07
- Amends: the configuration loading model of ADR-0019 (layered configuration) and the
  stage-2 shared format doc section 1.2/4.4 (org policy file strictness). ADR-0022
  (capability model, manifest schema 3) remains in force; this ADR fixes how the
  schema-3 policy file is LOADED, not what it says.

## Context

Live validation of stage 3 (2026-07-03, real machine, org-policy deployment) found that
the server cannot start with ANY policy file present at the org path
(`%ProgramData%\browser-mcp\policy.json` on Windows). Root cause: the policy file has
two owners with mutually exclusive schema gates.

- `governance::manifest::document::parse_manifest` gates `schema == 3` (ADR-0022
  Decision 6) and fully deserializes and validates the whole document, INCLUDING the
  `config` array (typed `Vec<ConfigEntry>`, per-entry key/value/level validation).
- `governance::config::load::parse_org_config` gates `schema == 2` and re-parses the
  same file's `config` array a second time, untyped. It was written for the
  pre-ADR-0022 schema-2 wire format and was never updated when the manifest moved to
  schema 3 (stage-3 task s05 bumped one gate and could not see the other, because
  there were two).

No schema value satisfies both gates: a schema-2 file dies in `parse_manifest`
(startup manifest load), a schema-3 file dies in `parse_org_config`
(`ConfigStore::load_initial_with_manifest_config`). The product actively steers users
into the failure: `policy init` prints instructions to place its (schema-3, validated)
output at exactly the org path.

The wider survey (2026-07-03) found the double-ownership is systemic, not a one-line
drift:

- Server startup parses the org file twice (once per parser). `doctor` reproduces the
  double parse. One `config list` invocation parses it up to four times
  (`read_layers` + `shadow_line`'s own `load_policy`). The hot-reload watcher
  re-parses it with the dead schema-2 parser on every settled change.
- `manifest_config_as_user_layer` exists only as a workaround for the double parse: it
  deliberately returns an empty map for an org-sourced manifest "because
  parse_org_config already reads the same file independently".
- Config-entry validation is implemented twice (typed `validate_config_entry` vs
  `parse_org_config`'s hand-rolled walk), with one real semantic delta: only the
  hand-rolled walk rejects duplicate keys.
- `load::load_and_resolve` is dead public API with zero callers; the startup path it
  claims to be is actually `ConfigStore::load_initial_with_manifest_config`.

Two parsers for one file is a defect class, not an incident: any future change to the
policy format re-opens it. The project value is fewer, more meaningful moving parts.

## Decision

### 1. `parse_manifest` is the sole authority for the policy file

One file kind, one parser, one schema gate. `parse_manifest` (schema `== 3`,
all-or-nothing strict) is the ONLY code that reads, parses, or validates a policy
file, regardless of origin (org path, `--manifest file://`, `BROWSER_MCP_MANIFEST`,
`env://`). Nothing else in the crate may inspect the raw bytes of a policy file.

`parse_org_config` is DELETED. `load_and_resolve` (dead) is DELETED.

### 2. Config layers derive from the parsed manifest

The org/user config layers consume the already-validated `Manifest.config` entries;
they never re-read the file:

- A pure function in `governance::config` derives today's `OrgConfig` shape (the
  `mandatory` and `recommended` layer maps) from `&[ConfigEntry]` by splitting on
  `Level`. `OrgConfig` itself and everything downstream of it (`layer_inputs`,
  `layers::resolve`, the strictness of later layers) are unchanged.
- `manifest_config_as_user_layer` inverts: `Manifest.config` becomes the ONLY channel
  for manifest-carried config, for both origins. Org-sourced manifest: entries split
  by level into the org layers. User-sourced manifest: ALL entries land in the user
  layer, with the existing mandatory-downgrade warning, exactly as today.
- `ConfigStore::load_initial*` takes the parsed policy (the `LoadedPolicy` produced
  once at startup) as input instead of performing its own org-file read. The user
  config file (`config.json`) is still read and parsed by `ConfigStore` as today.

### 3. Duplicate config keys become a manifest validation error

`parse_org_config`'s one semantic not already covered by manifest validation is
preserved by moving it: `parse_manifest` rejects a manifest whose `config` array
contains the same `key` twice (a Field-class validation error naming the duplicate
key). This now applies to user-sourced manifests too, which is a deliberate
tightening: a duplicated config key is equally a mistake in either origin.

### 4. The hot-reload watcher re-parses with the real parser

The watcher's org-path slot calls `parse_manifest` on change and re-derives the
config layers from the fresh `Manifest.config`. The plan_reload strictness matrix is
unchanged: startup is fail-loud (a present-but-invalid policy file stops the server);
reload keeps last-good state and logs at ERROR for the org file, WARN for the user
file. In this ADR the watcher consumes ONLY the config entries of the fresh parse;
grants/mode/hash remain startup-frozen (ADR-0025 lifts that separately -- the single
parse this ADR creates is deliberately the hook ADR-0025 attaches to).

### 5. The strictness matrix is two file KINDS, not two parsers per file

Unchanged and now structural: the policy file (a manifest) is all-or-nothing strict
via `parse_manifest`; the user config file (`config.json`, no schema member, object
map) keeps `parse_user_config`'s lenient-per-entry behavior. What this ADR removes is
the org file having two parsers; it does not merge the two file kinds.

### 6. One parse per consumer

Every load path performs exactly one parse of the policy file per invocation or
change event: server startup (one `load_policy`), doctor (one), `config` CLI
including `config list`'s shadow line (one, shared), presets (one), the watcher (one
per settled change). `policy explain` and `policy simulate` already parse a named
file once and are unchanged.

## Consequences

- Positive: the outage class dies structurally. There is no second schema gate to
  drift, ever. A schema-3 org policy with a config block loads; the stage-3 live
  validation backlog (BROWSER-TESTS s-live-1..4) becomes runnable.
- Positive: `policy init`'s printed instructions become true.
- Positive: fewer parts: two parsers, one dead function, and one workaround function
  become one parser plus one pure split.
- Positive: the watcher now holds a full fresh `Manifest` on every org change, which
  is exactly the seam manifest hot-reload (ADR-0025) needs.
- Negative: duplicate config keys in a user-sourced manifest go from silently
  accepted (last-write-wins through the untyped walk was never reached for user
  manifests; the typed path ignored duplicates) to a hard validation error. Accepted:
  strictness here is truthfulness.
- Negative: `ConfigStore`'s constructor surface changes shape (takes the parsed
  policy), which ripples through `server.rs`, `doctor.rs`, `cli.rs`, `presets.rs`,
  and their tests. Contained, mechanical, and the tests pin the new single-parse
  behavior.

## Future work (explicitly not this ADR)

- Manifest hot-reload (grants/mode/hash swapping live, re-advertisement): ADR-0025.
- Any change to what the policy file SAYS (schema 4, new grant fields): none planned;
  ADR-0022 owns the format.

## Provenance

Found during the stage-3 post-batch live validation attempt (2026-07-03): deploying a
schema-3 org policy to validate capability enforcement took the server down at
startup. Root-caused in session (user + Claude); the user directed the architectural
fix over the band-aid ("break-and-rebuild if it's the requirement for better, less
numerous but more meaningful moving parts"). User-decided: single-loader direction;
strict-tightening acceptance. Recommended-and-accepted: duplicate-key rule relocation;
watcher re-pointing with unchanged fail-closed matrix; deleting `load_and_resolve`.
