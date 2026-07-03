# G02: Layered configuration resolution and config file loading

## Goal

Implement the five-layer configuration model of `docs/tasks/stage-2/00-shared-format.md`
section 2: per-key precedence org-mandatory > user > org-recommended > preset default >
built-in Minimal. Load the user config file and the org policy file from the exact
per-platform paths of shared format sections 1.1 and 1.2, tolerating absence (the
all-open invariant), warning on per-entry problems in the user file, and failing with a
clear error on any violation in the org file. Resolution produces, for every registered
key, the triple (value, source layer, locked flag) of shared format section 2.1. The
`Config` the mcp-server role uses is built from that resolution at startup instead of
`Config::default()`. Pure functions plus unit tests for precedence order, locked
propagation, missing files, malformed files, and unknown keys.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file location, enum string, and layer name in this task comes from it
  verbatim. Read it before writing any code. Load-bearing sections here: 1.1 (user
  config file), 1.2 (org policy file), 2 (layer model), 2.1 (resolved triple), 3.2
  (value types), 3.4 (key set), 4.4 (manifest config entries).
- G01, the typed-registry task: it grows `KeyDef` in `src/policy/mod.rs` from the
  boolean seed (`minimal_default: bool`, one key) to the typed registry of shared format
  sections 3.2 to 3.4 (a typed default value per key, constraints such as uint ranges
  and enum variants, and the seven stage-2 keys). G02 consumes that registry; it does
  not define keys. PREREQUISITE CHECK: open `src/policy/mod.rs` first. If `KeyDef` still
  has only `key`, `description`, and `minimal_default: bool`, the prerequisite has not
  landed; STOP and report that G01 must land first. Do not fold registry growth into
  this task and do not improvise typed defaults here.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed. G05
  (classification) is independent of this task.

Because G01 reshapes `src/policy/mod.rs` before G02 runs, the "Current behavior" section
below records the tree as it stands at authoring time. Re-read every file named below
before editing it and integrate against what G01 actually produced (its field, type, and
accessor names), keeping the semantics specified here.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles (mcp-server and native-host) are separate OS processes bridged by
tokio-native named-pipe / UDS IPC.

Stage 1 (docs/tasks/release-1/) hardened the engine. This is stage 2, the governance
layer: a separable overlay (ADR-0013; all-open stays first-class), landed
observe-then-enforce (ADR-0018), configured through one typed key registry with layered
precedence (ADR-0019), with the org policy experience of ADR-0020 on top. G02 is the
ADR-0019 core: the layer machinery and the two configuration files. Everything later --
CLI display (G03), audit keys taking effect, org locks in the extension UI, presets --
reads through the resolver this task builds.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc is the reconciled single source for formats and names; use ITS names, never
improvised ones. In particular the layer sources are exactly the strings
`org_mandatory`, `user`, `org_recommended`, `preset`, `builtin` (shared format 2.1).

Key files for this task:

- `src/policy/mod.rs` -- the governance module root; holds the typed key registry
  (`KeyDef`, `KEYS`, `Config`) after G01. Your new modules are declared here.
- `src/policy/redact.rs` -- existing overlay example (style reference only; do not
  modify).
- `src/mcp/server.rs` -- the JSON-RPC loop; builds the session `Config` today at line 28
  (`let config = Config::default();`) and threads it to `handle_tools_call`.
- `src/main.rs` -- role selection. Only the mcp-server role (`run_server`, line 230,
  which calls `browser_mcp::mcp::server::run(browser)` at line 254) may load
  configuration. The native-host role (`run_native_host_role`, line 212) is a stateless
  relay and must not; the installer subcommands must not.
- `src/error.rs` -- the typed library error (`thiserror`). It has no configuration
  variant yet.
- `Cargo.toml` -- already has `dirs = "6"`, `serde` (derive), and `serde_json` with
  `preserve_order`. G02 needs no new dependency.

## Current behavior

All facts verified against the working tree at authoring time (before G01):

- `src/policy/mod.rs` (104 lines) holds the seed registry: `KeyDef` (lines 25-33) with
  `key`, `description`, `minimal_default: bool`; one registered key,
  `content.security.secrets.redact` (lines 39-47); `Config` (lines 51-70) with one field
  `secrets_redact`, `Config::minimal()`, and `Default` delegating to `minimal()`
  (lines 72-76). It declares `pub mod redact;` (line 19) and nothing else.
- No code anywhere reads a `config.json` or `policy.json` for this binary. The only
  `dirs::config_dir()` use is the installer resolving MCP client config paths
  (`src/install/mod.rs` line 79). There is no layer model, no `Source` type, no
  resolver, and no loader.
- `src/mcp/server.rs` `run` (line 22) builds `let config = Config::default();`
  (line 28) under a comment saying the policy engine resolves this per session when it
  lands. `handle_line` (line 55) and `handle_tools_call` (line 116) thread `config` by
  value (`Config` is currently `Copy`); the only consumer is
  `config.secrets_redact()` at line 142.
- `src/main.rs` parses `--manifest` (lines 33-35) but `run_server` only logs it
  (lines 231-235); nothing parses a manifest. `BROWSER_MCP_MANIFEST` is not read.
- `src/error.rs` `Error` has variants for protocol, native messaging, IPC, session,
  JSON, IO, and installer failures; nothing for configuration. `pub type Result<T>`
  (line 61) is the library-wide alias.
- `tests/mcp_protocol.rs` `initialize_tools_list_and_tool_call_over_stdio` (line 48)
  spawns the real binary with no manifest and asserts the all-open surface: 13 tools and
  `list["result"]` equal to the parsed sacred fixture (lines 74-78). It must keep
  passing UNCHANGED.

## Required behavior

### 1. New module `src/policy/layers.rs` (the layer model and resolver)

Declare `pub mod layers;` in `src/policy/mod.rs` next to the existing module
declarations. The module is pure: no filesystem, no environment, no tracing. Module doc
comment: this is the ADR-0019 layer model (shared format section 2); precedence is
org-mandatory > user > org-recommended > preset default > built-in Minimal; layer 5
always defines every key, so resolution never fails.

Types (doc comments on every public item):

```rust
/// Which layer a resolved value came from (shared format section 2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    OrgMandatory,
    User,
    OrgRecommended,
    Preset,
    Builtin,
}
```

`Source::as_str(&self) -> &'static str` returns exactly `"org_mandatory"`, `"user"`,
`"org_recommended"`, `"preset"`, `"builtin"`. These strings are the shared-format 2.1
`source` enum consumed later by `config list`, the extension options page, and audit
tooling; the doc comment must say so.

```rust
/// The resolved triple for one key (shared format section 2.1).
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The effective value, already validated against the key's type and constraints.
    pub value: serde_json::Value,
    /// The layer that defined it.
    pub source: Source,
    /// True if and only if `source` is `Source::OrgMandatory`.
    pub locked: bool,
}
```

```rust
/// Per-layer candidate values keyed by dotted key name. Entries are validated by the
/// loaders before they get here; `resolve` only picks, it does not re-validate.
#[derive(Debug, Clone, Default)]
pub struct LayerInputs {
    pub org_mandatory: serde_json::Map<String, serde_json::Value>,
    pub user: serde_json::Map<String, serde_json::Value>,
    pub org_recommended: serde_json::Map<String, serde_json::Value>,
    /// Layer 4. Structurally supported and tested here, but left EMPTY by this task's
    /// loader: mapping a preset name to per-key defaults is the presets task (G18).
    pub preset: serde_json::Map<String, serde_json::Value>,
}
```

(`serde_json::Map` is order-preserving here because `Cargo.toml` enables
`preserve_order`.)

```rust
/// The full resolution: one triple per registered key, in `KEYS` registry order.
#[derive(Debug, Clone)]
pub struct Resolution { /* private: Vec<(&'static str, Resolved)> */ }
```

`Resolution` exposes `pub fn get(&self, key: &str) -> Option<&Resolved>` and
`pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Resolved)>` (registry
order; G03's `config list` depends on this order being stable).

The resolver:

```rust
/// Resolve every registered key against the five layers. Infallible: the built-in
/// layer (the registry defaults) always defines every key.
pub fn resolve(layers: &LayerInputs) -> Resolution
```

For each `KeyDef` in `KEYS`, in order: check `org_mandatory`, then `user`, then
`org_recommended`, then `preset`; the first map containing the key wins, with the
matching `Source` and `locked = (source == Source::OrgMandatory)`. If no map contains
the key, the value is the registry's built-in default converted to `serde_json::Value`,
with `Source::Builtin` and `locked = false`. If G01 did not already provide a
KeyValue-to-JSON conversion, add one small helper for it (in `layers.rs` or next to the
value type, wherever G01 put it).

Value validation: the loaders (section 2 below) validate every file-sourced value
against the key's type and constraints per shared format section 3.2. If G01 already
exposes a validation function taking a key definition and a `serde_json::Value`, call
it. Otherwise implement in `layers.rs`:

```rust
/// Validate a candidate value against a key's declared type and constraints
/// (shared format section 3.2). Err carries a human-readable reason, e.g.
/// "expected an integer between 0 and 60000".
pub fn validate_value(def: &KeyDef, value: &serde_json::Value) -> Result<(), String>
```

Rules, exactly section 3.2:

- bool: must be a JSON boolean.
- uint: must be a JSON number readable as `u64` (`Value::as_u64`; this correctly
  rejects signs, fractions, and exponent forms), then within the key's declared
  min/max bounds.
- enum: must be a JSON string equal (case-sensitive) to one of the key's declared
  variants.
- string: must be a JSON string. Do NOT add per-key extras such as absolute-path
  checking for `audit.file.path`; that belongs to the audit task that consumes it.
- string list: must be a JSON array whose elements are all strings, with no duplicate
  elements; order preserved. For the domain-pattern-list constraint on
  `content.security.sacred_domains`, validate SHAPE ONLY here (array of unique strings)
  unless G01 already implemented pattern-grammar validation; the section-5 pattern
  grammar belongs to the domain-matching task. Leave an ASCII code comment saying so.
- any other JSON shape (null, object, mixed array): invalid for every key.

### 2. New module `src/policy/load.rs` (paths, parsing, orchestration)

Declare `pub mod load;` in `src/policy/mod.rs`. Module doc comment: loads the two
configuration files of shared format section 1, applies the per-file strictness matrix
(lenient user file, strict org file), and produces the `LayerInputs` for
`layers::resolve`. Parsing is pure over `&str` so unit tests never touch the real
filesystem; only the two thin path/read functions and `load_and_resolve` do I/O.

#### 2.1 Paths (shared format sections 1.1 and 1.2, verbatim)

```rust
/// Path of the user config file; None when the platform config dir is unavailable.
pub fn user_config_path() -> Option<std::path::PathBuf>
```

Returns `dirs::config_dir()?.join("browser-mcp").join("config.json")`. This lands on
exactly the shared-format 1.1 table: `%APPDATA%\browser-mcp\config.json` on Windows,
`~/Library/Application Support/browser-mcp/config.json` on macOS,
`~/.config/browser-mcp/config.json` on Linux.

```rust
/// Path of the org policy file (fixed per platform; shared format section 1.2).
pub fn org_policy_path() -> std::path::PathBuf
```

Per platform, with `cfg` branches:

- Windows: `%ProgramData%\browser-mcp\policy.json`, reading the `ProgramData`
  environment variable and falling back to `C:\ProgramData` when unset.
- macOS (`target_os = "macos"`): `/Library/Application Support/browser-mcp/policy.json`.
- Linux and other unix: `/etc/browser-mcp/policy.json`.

There is NO flag, environment variable, or config key that relocates or bypasses the
org policy path (shared format 1.2: "No flag can bypass it"). Do not add one.

#### 2.2 Parsing the user config file (lenient per entry, strict per file)

The file format (shared format 1.1):

```json
{
  "preset": "safe",
  "config": {
    "content.security.sacred_domains": ["mybank.com", "*.mybank.com"],
    "audit.destination": "file"
  }
}
```

```rust
/// The parsed user config file (shared format section 1.1).
#[derive(Debug, Clone, Default)]
pub struct UserConfig {
    /// Validated preset name if one was declared: "fully_open", "safe", or
    /// "restricted". Retained for the presets task (G18); NOT applied to any layer
    /// by this task.
    pub preset: Option<String>,
    /// Validated user-layer values by dotted key name.
    pub values: serde_json::Map<String, serde_json::Value>,
}

/// Parse the user config file content. `path` is used only in messages.
/// Returns the parsed file plus per-entry warnings for the caller to log.
pub fn parse_user_config(content: &str, path: &str) -> crate::Result<(UserConfig, Vec<String>)>
```

Behavior:

- Strip one leading UTF-8 BOM (`\u{FEFF}`) if present, then parse as JSON.
- File-shape violations are hard errors (the new `Error::Config` variant, section 4
  below), each with a message naming `path` and the problem: content that is not valid
  JSON; a top level that is not a JSON object; a `preset` member that is present but
  not a string; a `config` member that is present but not an object.
- Per-entry problems inside `config` are warnings, never errors (shared format 1.1:
  "the rest of the file still loads"). For each entry:
  - key not in the registry (`KEYS`): push a warning naming the key, skip the entry;
  - value fails `validate_value`: push a warning naming the key and the reason, skip
    the entry;
  - otherwise: insert into `UserConfig.values`.
- `preset` present but not one of `fully_open`, `safe`, `restricted`: push a warning
  naming the bad value, treat as absent.
- Unknown top-level members other than `preset` and `config`: push a warning naming
  the member, ignore it.
- Duplicate members inside the JSON `config` object follow serde_json semantics (last
  one wins); no extra handling.

Rationale to carry as a code comment: the user file is user-serviceable, so one bad
entry must not take the whole session down; but an unreadable or structurally broken
file is a hard error because silently continuing without the user's own settings (for
example a sacred-domains list) would be fail-open on a user-authored protection, and
the engine is truthful.

#### 2.3 Parsing the org policy file (strict everywhere)

The org policy file is the organization manifest (shared format section 4). G02
consumes ONLY two members: `schema` and the `config` array. Grants, `name`, `version`,
`mode`, and `identity` are parsed by the manifest tasks (G12+); do not read them, do not
validate them, and do not warn about them here.

Config entry shape (shared format 4.4):

```json
{ "key": "audit.enabled", "value": true, "level": "mandatory" }
```

```rust
/// The org-layer values extracted from the org policy file (shared format 1.2, 4.4).
#[derive(Debug, Clone, Default)]
pub struct OrgConfig {
    /// Entries with "level": "mandatory" -- layer 1, locked.
    pub mandatory: serde_json::Map<String, serde_json::Value>,
    /// Entries with "level": "recommended" -- layer 3.
    pub recommended: serde_json::Map<String, serde_json::Value>,
}

/// Parse the org policy file content. `path` is used only in messages.
pub fn parse_org_config(content: &str, path: &str) -> crate::Result<OrgConfig>
```

EVERY violation is a hard `Error::Config` with a message naming `path`, the offending
key or entry index, and the problem. No warn-and-continue: org policy that cannot be
applied exactly must stop the server rather than silently degrade. Violations:

- content that is not valid JSON (after BOM strip), or a top level that is not an
  object;
- `schema` missing, not an integer, or not equal to `2` (shared format 4.1: the binary
  rejects unknown schema versions with a clear error);
- `config` present but not an array; an entry that is not an object; an entry member
  other than `key`, `value`, `level`; `key` or `level` missing or not strings;
- `key` not in the registry (shared format 4.4: unknown keys are a validation error);
- `value` failing `validate_value` (include the reason in the message);
- `level` not exactly `"mandatory"` or `"recommended"`;
- the same `key` appearing in more than one entry (any levels).

`config` absent is valid and yields empty maps (an org file may, once G12 lands, carry
only grants).

#### 2.4 Orchestration

```rust
/// Load both configuration files from their platform paths, log warnings, and resolve
/// all layers. Called once at mcp-server startup. Absence of either file is normal.
pub fn load_and_resolve() -> crate::Result<layers::Resolution>
```

- Read each path with `std::fs::read_to_string`. `ErrorKind::NotFound` (including a
  missing parent directory, and a `None` from `user_config_path()`) means the file is
  absent: use the empty default for that side. ANY other I/O error (for example
  permission denied) is a hard `Error::Config` naming the path: an org policy file
  that exists but cannot be read must not silently yield an all-open session.
- Parse with the pure functions above. Emit every returned user-file warning with
  `tracing::warn!` (tracing writes to stderr; stdout stays reserved for JSON-RPC).
- If `UserConfig.preset` is `Some(name)`, emit exactly one
  `tracing::warn!` stating that preset `<name>` is declared in the user config file
  but preset defaults are not implemented yet, so it has no effect. Observing must
  never present as protection; a declared-but-inert preset must be said out loud.
- Build `LayerInputs { org_mandatory: org.mandatory, user: user.values,
  org_recommended: org.recommended, preset: Map::new() }` and return
  `layers::resolve(&inputs)`.
- Synchronous `std::fs` at startup is fine (one-time, small files); do not add async
  file I/O.

### 3. `Config::from_resolution`

In the file where `Config` is defined (today `src/policy/mod.rs`; follow G01's layout):

```rust
/// Build the typed session Config from a resolution. Values in the resolution are
/// already validated, so conversion cannot fail; an impossible mismatch falls back
/// to the registry default (debug_assert, documented unreachable-by-construction).
pub fn from_resolution(resolution: &layers::Resolution) -> Config
```

Map each registered key's resolved `serde_json::Value` to the corresponding typed
`Config` field using the field and accessor names G01 established (booleans via
`as_bool`, uints via `as_u64`, enums and strings via `as_str` to owned values, string
lists by collecting the string elements). Keep `Config::minimal()` and the existing
registry-agreement test working: `from_resolution(&resolve(&LayerInputs::default()))`
must equal `Config::minimal()` field for field. If `Config` does not yet derive
`PartialEq`, derive it so tests can compare whole values. If G01 made `Config`
non-`Copy` (owned strings or lists), keep whatever threading G01 chose in
`src/mcp/server.rs`; if it is still `Copy`-threaded and your change breaks that, switch
`handle_line` / `handle_tools_call` to take `&Config` with minimal churn.

### 4. New error variant

Add exactly one variant to `Error` in `src/error.rs`:

```rust
/// A configuration file failed to load or validate (user config or org policy file).
#[error("configuration error: {0}")]
Config(String),
```

Messages passed in must be self-contained: file path plus what is wrong plus, for value
violations, the expected type or constraint (the `validate_value` reason).

### 5. Startup wiring in `src/mcp/server.rs`

Replace the current line 28 block

```rust
let config = Config::default();
```

(and its three-line comment above) with:

```rust
let resolution = policy::load::load_and_resolve()?;
let config = Config::from_resolution(&resolution);
```

plus a short ASCII comment: layered configuration per ADR-0019 (org-mandatory > user >
org-recommended > preset > built-in Minimal); with no files present this resolves to
the built-in defaults and behavior is byte-identical to all-open. `run` already returns
`crate::Result<()>`, so a load failure propagates out, `main` prints the
`configuration error: ...` message, and the server exits before answering any request.
That is the intended fail-loud behavior for org-file violations and structurally broken
user files.

Nothing else in the server changes. Only the mcp-server role loads configuration:
`run_native_host_role` and the installer subcommands in `src/main.rs` must not call
`load_and_resolve` (do not touch `src/main.rs` at all).

### 6. Strictness matrix (normative summary)

| Condition | User config file | Org policy file |
|---|---|---|
| File absent | empty layer, no message | empty layers, no message |
| Unreadable (I/O error other than not-found) | error | error |
| Invalid JSON / top level not an object | error | error |
| `preset` not a string / `config` wrong JSON type | error | error (`config` not an array) |
| Unknown dotted key in an entry | warn, skip entry | error |
| Value fails type/constraint validation | warn, skip entry | error |
| Unknown preset name | warn, treat as absent | n/a |
| Bad or missing `level` | n/a | error |
| Duplicate key across entries | n/a (JSON object; last wins) | error |
| `schema` missing or not 2 | n/a (no schema member) | error |

This matrix reconciles shared format 1.1 (user file: "rejected at load with a warning
naming the key; the rest of the file still loads") with shared format 4.4 (manifest
config entries: validation errors). If any cell seems to contradict the shared format
doc, stop and report; do not guess.

### 7. Unit tests

Inline `#[cfg(test)] mod tests` in `layers.rs` and `load.rs`. All pure; no real
filesystem, no environment mutation (path functions are exercised read-only). Required
tests, by name and assertion:

In `layers.rs`:

1. `builtin_layer_defines_every_key_when_inputs_are_empty`: `resolve` over
   `LayerInputs::default()` yields, for every key in `KEYS`, `Source::Builtin`,
   `locked == false`, and the registry default converted to JSON.
2. `precedence_walks_org_mandatory_user_org_recommended_preset_builtin`: for one key
   (use `content.security.secrets.redact` or any registered key), populate all four
   input maps with distinguishable values; assert org_mandatory wins; then remove the
   top layer one at a time and assert user, then org_recommended, then preset, then
   builtin win in turn. This exercises the preset slot even though the loader leaves
   it empty.
3. `locked_is_true_iff_source_is_org_mandatory`: same key resolved from each of the
   five sources; `locked` is true exactly once, for org_mandatory.
4. `source_strings_match_the_shared_format_enum`: `as_str` returns exactly
   `org_mandatory`, `user`, `org_recommended`, `preset`, `builtin`.
5. `resolution_iterates_in_registry_order`: keys from `Resolution::iter` equal the
   keys of `KEYS` in order.
6. `validate_value_enforces_section_3_2`: accepted and rejected samples per type:
   bool vs `"yes"`; uint in range vs negative, fractional (`5.5`), exponent (`1e3`),
   and out-of-range values; enum variant vs wrong case / unknown variant; string vs
   number; string list vs mixed array and vs duplicate elements; `null` and `{}`
   rejected for every type.

In `load.rs`:

7. `missing_files_resolve_to_builtin_and_config_equals_minimal`: empty
   `LayerInputs` (as the loader would produce with both files absent) resolves so that
   `Config::from_resolution(..)` equals `Config::minimal()`. This is the all-open
   invariant test.
8. `malformed_user_file_is_an_error`: each of `not json`, `[]`, `{"preset": 3}`,
   `{"config": []}` makes `parse_user_config` return `Err`, and the message contains
   the path string passed in.
9. `unknown_user_key_warns_and_is_skipped`: a `config` map with one unknown key and
   one valid key yields values containing only the valid key, plus exactly one warning
   naming the unknown key.
10. `invalid_user_value_warns_and_is_skipped`: a registered key with a type-invalid
    value is skipped with a warning naming the key; a valid sibling entry still loads.
11. `unknown_preset_warns_and_is_treated_as_absent`: `{"preset": "extreme"}` parses
    Ok with `preset == None` and one warning; `{"preset": "safe"}` parses Ok with
    `preset == Some("safe".into())` and no warnings.
12. `org_entries_populate_layers_by_level`: an org file with one `mandatory` and one
    `recommended` entry lands each in the right map; after `resolve`, the mandatory
    key has `Source::OrgMandatory` and `locked == true`, the recommended key has
    `Source::OrgRecommended` and `locked == false`, and a user-layer value for the
    recommended key overrides it while one for the mandatory key does not.
13. `org_file_violations_are_errors`: each of the following returns `Err` from
    `parse_org_config`: invalid JSON; `schema` missing; `schema: 3`; `schema: "2"`;
    `config` not an array; an entry with an unknown key; an entry with a type-invalid
    value; an entry with `level: "optional"`; an entry missing `level`; two entries
    with the same key; an entry with an extra member. Error messages name the path and
    the offending key or index.
14. `paths_follow_the_shared_format_locations`: `cfg`-gated per platform. On Windows,
    `user_config_path` ends with `browser-mcp\config.json` under `dirs::config_dir()`
    and `org_policy_path` ends with `browser-mcp\policy.json` under the `ProgramData`
    root. On macOS, the org path equals
    `/Library/Application Support/browser-mcp/policy.json`; on Linux,
    `/etc/browser-mcp/policy.json`. Read-only assertions; do not set or unset
    environment variables in tests.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. This task does not touch tool
   advertisement or schemas at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task changes no file under `extension/`.
3. All-open stays first-class: with no config files, no manifest, and default config,
   behavior is byte-identical to today. Resolution with empty inputs must equal
   `Config::minimal()`, and `tests/mcp_protocol.rs` must pass unchanged. Test the
   invariant (test 7).
4. ASCII only in ALL code, comments, strings, and docs: no em-dashes, arrows, or curly
   quotes.
5. The engine is truthful: a broken org policy file stops the server with a clear
   error instead of silently running open; a structurally broken user file stops the
   server instead of silently dropping the user's own settings; a declared preset that
   has no effect yet is warned about; every skipped user entry is warned about. Never
   swallow a problem silently.
6. No new runtime dependencies. `dirs`, `serde`, `serde_json` (with `preserve_order`),
   `tracing`, and `thiserror` are already in `Cargo.toml` and are all this task needs.
   Do not add `config`, `figment`, `toml`, `directories`, or any other crate.
7. Rust 2021 edition; typed errors via `thiserror` (the one new `Error::Config`
   variant); doc comments on every public item; `cargo fmt` clean;
   `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline; no new
   integration test file is needed for this task.
8. Do NOT copy code from other projects; implement from the behavior described here.
9. Use the shared format doc's names verbatim: source strings `org_mandatory`, `user`,
   `org_recommended`, `preset`, `builtin`; triple fields value / source / locked; file
   names `config.json` and `policy.json` under a `browser-mcp` directory; entry fields
   `key` / `value` / `level`; levels `mandatory` / `recommended`; presets `fully_open`
   / `safe` / `restricted`.
10. The org policy path is fixed. Do not add any flag, environment variable, or key
    that moves or disables it. (The IPC endpoint override `BROWSER_MCP_ENDPOINT` used
    by tests is unrelated; leave it alone.)
11. Loading is read-only. This task never creates, writes, or migrates either file;
    writing the user config file is the `config set` surface (G03 and the
    native-messaging settings task).
12. stdout is reserved for the JSON-RPC stream. All warnings and errors go through
    `tracing` (stderr) or the returned `Error`.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including the fourteen new tests above,
   `tests/tool_schema_fidelity.rs` unchanged, and `tests/mcp_protocol.rs` unchanged.
   (The protocol tests spawn the real binary, which now reads the real platform paths:
   run on a machine where `%ProgramData%\browser-mcp\policy.json` / the unix org path
   does not exist and any personal `config.json` is valid or absent. If a leftover
   experiment file breaks them, that is the loader working as specified; remove the
   file.)
4. `git status` / `git diff --stat` shows changes ONLY to: `src/policy/mod.rs`
   (module declarations, `Config::from_resolution`, possibly a `PartialEq` derive),
   the new `src/policy/layers.rs`, the new `src/policy/load.rs`, `src/error.rs` (one
   variant), and `src/mcp/server.rs` (the startup resolution block). `src/main.rs`,
   `src/dispatch.rs`, `extension/`, and `src/mcp/schemas/tools.json` show no diff.
5. Grep the changed files for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/policy src/error.rs src/mcp/server.rs`); there must be
   none.
6. Manual smoke (optional but recommended): create the user config file at the
   platform path with `{"config": {"no.such.key": true}}`, start the binary with
   stderr visible, confirm one warning naming `no.such.key` and a normally working
   server; replace the content with `not json`, confirm the server exits with a
   `configuration error:` message naming the path; delete the file afterward.

Build note: if `target/debug/browser-mcp.exe` is locked by a running MCP session,
rename it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`)
and rebuild. Binary changes need an MCP client restart to take effect; no extension
reload is needed for this task.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Manifest grants and everything else in the org policy file beyond `schema` and the
  `config` array: no grant parsing, no `Grant` type, no domain matching, no manifest
  content hash, no `name` / `version` / `mode` / `identity` reading or validation, no
  denials. Those are G12+ tasks.
- `--manifest` / `BROWSER_MCP_MANIFEST` handling, including the section 1.3 selection
  rule and the mandatory-to-user downgrade for user-supplied manifests. `--manifest`
  stays parsed-and-logged exactly as today; do not touch `src/main.rs`.
- Presets (G18): no mapping from a preset name to layer-4 values, no preset selection
  CLI or UI, no per-preset behavior of any kind. This task only parses, validates, and
  retains the `preset` field (with the inert-preset warning) and keeps the resolver's
  layer-4 slot working via synthetic test inputs.
- CLI display and editing (G03): no `config list` / `config get` / `config set`
  subcommands, no new clap surface, no human-readable rendering of the resolution.
- The native-messaging settings protocol (shared format section 9): no `get_status` /
  `get_config` / `set_config_key` messages.
- Live file watching, hot reload, SIGHUP handling, or mid-session re-resolution. Both
  files are read exactly once at mcp-server startup; a change takes effect on the next
  session.
- Audit subsystem work: the `audit.*` keys resolve like any other key, but nothing
  consumes them yet (G06+); do not create audit records, files, or destinations, and
  do not validate `audit.file.path` beyond "is a JSON string".
- Enforcement and dispatch changes: `src/dispatch.rs` keeps its no-op `policy_check`
  and `audit` seams untouched; no sacred-domain enforcement (that task consumes the
  resolved key later); no tool advertisement filtering (G14).
- JSON Schema generation for manifests or configs (an ADR-0020 task).
- Writing, creating, or migrating either configuration file, or creating their
  directories.
- Deprecation aliases for key renames (shared format 3.1); no key is renamed here.
