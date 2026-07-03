# G01: Typed key registry: value types beyond bool

## Goal

Grow the governance configuration registry in `src/policy/mod.rs` from bool-only to the
full typed value model defined in `docs/tasks/stage-2/00-shared-format.md` section 3
(bool, uint with min/max, enum, string, string list), register the seven-key stage-2
initial key set with per-preset defaults, and add validation helpers that parse a JSON
value against a key definition with typed errors. `KEYS` stays the single static registry
driving everything: the `Config` struct is built FROM the registry, never from duplicated
literals. Wire `engine.connection.first_call_wait_ms` into the bounded first-call wait
that release-1 task T04 introduced. No file loading, no CLI, no manifest logic: this task
produces the typed registry primitive that G02 (layer loading), G03 (CLI), and the rest
of stage 2 consume.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` (sections 2, 3.1, 3.2, 3.3, 3.4, 5.1, and the
  9.2 error-message vocabulary). Read it before writing any code; its names are
  authoritative and every name in this prompt comes from it.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed, in
  particular T04 (`docs/tasks/release-1/t04-first-call-warmup-bounded-wait.md`), which
  introduced the `FIRST_CALL_WAIT_MS` constant this task replaces. If T04 has NOT landed
  in your tree, follow the explicit fallback in Required behavior part 6.
- No other stage-2 task is a prerequisite. G02, G03, G05, G06 build on this one.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled on tokio, no MCP SDK crate) and the Chrome
native-messaging host; a thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance layer per
ADR-0013 (separable overlay; all-open stays first-class), ADR-0018 (observe-then-enforce
sequencing), ADR-0019 (layered configuration and typed key registry), and ADR-0020 (org
policy experience). ADR-0019 is the driver here: one typed key registry is the single
source of truth for every configurable behavior (names, types, constraints, descriptions,
per-preset defaults), and it later drives the CLI, the extension options page, the
generated JSON Schema, and the docs. This task builds that registry; it deliberately does
NOT build any of its consumers.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc `docs/tasks/stage-2/00-shared-format.md` is the reconciled single source for
formats and names. Use its key names, preset names, type names, and error vocabulary
exactly; do not improvise alternatives.

Key files for this task:

- `src/policy/mod.rs` -- the governance module and the registry seed (`KeyDef`, `KEYS`,
  `Config`). Almost all changes land here.
- `src/policy/pattern.rs` -- NEW file you create: syntactic domain-pattern validation.
- `src/policy/redact.rs` -- existing overlay consumer of `Config::secrets_redact()`
  (style reference only; do not modify).
- `src/mcp/server.rs` -- threads `Config` through `handle_line` / `handle_tools_call`
  and (post-T04) holds the bounded-wait constant you replace.
- `src/lib.rs` -- already declares `pub mod policy;` (line 23). No change needed.
- `Cargo.toml` -- already has `serde_json` (with `preserve_order`), `thiserror` 2,
  `tokio`, `clap`, `tracing`. No change allowed.

Files you will read but MUST NOT modify: `src/mcp/schemas/tools.json` (sacred,
byte-frozen tool schemas), `tests/tool_schema_fidelity.rs` (guard test),
`src/dispatch.rs`, `src/policy/redact.rs`, everything under `extension/`.

Build and test: `cargo test` from the repo root (this is a Windows dev machine; the same
commands work on Unix). Also run `cargo fmt` and
`cargo clippy --all-targets -- -D warnings`. If `target/debug/browser-mcp.exe` is locked
by a running session, rename it aside first (for example
`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild.

## Current behavior

Verified against the working tree BEFORE the release-1 tasks land; T04 restructures
`src/mcp/server.rs`, so re-verify line numbers in your tree before editing.

- `src/policy/mod.rs` lines 25-33: `KeyDef` has exactly three fields: `key`,
  `description`, and `minimal_default: bool`. It cannot represent uint, enum, string, or
  string-list values, and it carries a single default instead of one per preset.
- `src/policy/mod.rs` lines 43-47: `KEYS` registers exactly one key,
  `content.security.secrets.redact` (const `CONTENT_SECURITY_SECRETS_REDACT`, line 39),
  with `minimal_default: true`.
- `src/policy/mod.rs` lines 51-76: `Config` derives `Debug, Clone, Copy`, holds one field
  `secrets_redact: bool`, hardcodes `secrets_redact: true` in `Config::minimal()`, and
  exposes `secrets_redact()`. `Default` delegates to `minimal()`.
- `src/policy/mod.rs` lines 78-103: two unit tests exist:
  `minimal_config_matches_the_registry_defaults` (pins `Config::minimal()` to the
  registry) and `every_key_name_is_dotted_and_unique`.
- `src/mcp/server.rs` line 12 imports `crate::policy::{self, Config}`; line 28 builds
  `let config = Config::default();` once per server run; line 55 `handle_line` takes
  `config: Config` by value (works because `Config` is `Copy`); lines 116-121
  `handle_tools_call` also takes `config: Config`; line 142 is the only read:
  `policy::redact::apply_to_result(&mut result, config.secrets_redact());`.
- There is no `first_call_wait_ms` anywhere in `src/` today (grep verified). Release-1
  T04 adds `const FIRST_CALL_WAIT_MS: u64 = 5000;` near `PROTOCOL_VERSION` in
  `src/mcp/server.rs` and uses it in two places: the initialize-arm warmup watcher and
  the bounded wait in `handle_tools_call`, both as
  `Duration::from_millis(FIRST_CALL_WAIT_MS)`. Its doc comment says it is "Slated to
  become governance config key `engine.connection.first_call_wait_ms` per ADR-0019". This
  task is that plumbing.
- `src/policy/pattern.rs` does not exist.
- `Cargo.toml`: `serde_json = { version = "1", features = ["preserve_order"] }` and
  `thiserror = "2"` are present. `sha2`, `uuid`, and `chrono` are NOT present; the shared
  format doc's crate note assigns them to LATER stage-2 tasks (manifest hash, audit), not
  to this one.
- `tests/` contains `mcp_protocol.rs`, `peer_death.rs`, `tool_schema_fidelity.rs`. None
  of them references `Config` directly; the only `Config` consumers are
  `src/mcp/server.rs` and `src/policy/mod.rs` itself.

## Required behavior

Seven parts. Implement all of them exactly as specified. Everything except part 5 lands
in `src/policy/mod.rs`.

### 1. Value and constraint types

Add these types, each with a doc comment. Names come from the shared format doc
section 3.3; the derives and helper methods below are normative for this task.

```rust
/// A configuration preset: a named bundle of layer-4 defaults (shared format doc
/// section 2). The built-in Minimal defaults (layer 5) equal the Safe preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    FullyOpen,
    Safe,
    Restricted,
}

impl Preset {
    /// The wire/file name of this preset: "fully_open", "safe", or "restricted".
    pub fn as_str(&self) -> &'static str { ... }

    /// Parse a preset name as written in config files. Returns `None` for unknown names.
    pub fn from_name(name: &str) -> Option<Preset> { ... }
}
```

```rust
/// A statically-declared default value for a registry key (one per preset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValue {
    Bool(bool),
    Uint(u64),
    Enum(&'static str),
    Str(&'static str),
    StrList(&'static [&'static str]),
}
```

```rust
/// An owned, validated configuration value at runtime. Produced by
/// [`KeyDef::parse_value`] and by converting a static [`KeyValue`] default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    Bool(bool),
    Uint(u64),
    Enum(String),
    Str(String),
    StrList(Vec<String>),
}
```

- Implement `From<KeyValue> for ConfigValue` (each variant maps to its owned twin).
- Implement `ConfigValue::to_json(&self) -> serde_json::Value` (Bool to JSON bool, Uint
  to JSON number, Enum and Str to JSON string, StrList to JSON array of strings). Later
  tasks (config list, get_config) render values through this.

```rust
/// The value type of a registry key, in the shared format doc's type vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    Bool,
    Uint,
    Enum,
    Str,
    StrList,
}

impl KeyType {
    /// The wire name: "bool", "uint", "enum", "string", or "string_list"
    /// (shared format doc section 9.2 vocabulary).
    pub fn name(&self) -> &'static str { ... }
}
```

```rust
/// Extra validation attached to a key beyond its base type (shared format doc 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyConstraint {
    /// Base-type check only.
    None,
    /// Uint keys: inclusive bounds. Every Uint key MUST declare this.
    UintRange { min: u64, max: u64 },
    /// Enum keys: the closed set of legal variants. Every Enum key MUST declare this.
    EnumVariants(&'static [&'static str]),
    /// Str keys: the empty string, or an absolute filesystem path
    /// (std::path::Path::is_absolute).
    EmptyOrAbsolutePath,
    /// StrList keys: each element must be a valid domain pattern
    /// (crate::policy::pattern::is_valid_pattern; shared format doc 5.1).
    DomainPatternList,
}
```

### 2. KeyDef growth

Replace the current three-field `KeyDef` with (keep `#[derive(Debug, Clone, Copy)]`; the
doc comments below are required in spirit, adjust wording to fit but keep them ASCII):

```rust
/// A governance configuration key: a stable dotted name, a human description, its
/// validation constraint, and one default per preset. The static [`KEYS`] table is the
/// single source of truth for the whole configurable surface (ADR-0019); the CLI,
/// extension UI, JSON Schema, and docs are all generated from it.
#[derive(Debug, Clone, Copy)]
pub struct KeyDef {
    /// Stable dotted identifier, e.g. `content.security.secrets.redact`.
    pub key: &'static str,
    /// What the key governs. Surfaced verbatim by config UIs.
    pub description: &'static str,
    /// Validation beyond the base type.
    pub constraint: KeyConstraint,
    /// Default under the "fully_open" preset.
    pub default_fully_open: KeyValue,
    /// Default under the "safe" preset. The built-in Minimal defaults equal these.
    pub default_safe: KeyValue,
    /// Default under the "restricted" preset.
    pub default_restricted: KeyValue,
}
```

Add these methods on `KeyDef` (doc comments required):

- `pub fn default_for(&self, preset: Preset) -> KeyValue` -- returns the matching
  default field.
- `pub fn key_type(&self) -> KeyType` -- derived from the `default_safe` variant
  (Bool -> Bool, Uint -> Uint, Enum -> Enum, Str -> Str, StrList -> StrList). A
  registry-integrity unit test (part 7) guarantees all three defaults share one variant,
  so deriving from `default_safe` is sound.
- `pub fn parse_value(&self, value: &serde_json::Value) -> Result<ConfigValue, ConfigValueError>`
  -- part 4.

Add a free lookup function:

```rust
/// Look up a key definition by its dotted name. `None` for unregistered names.
pub fn key_def(key: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|k| k.key == key)
}
```

The field `minimal_default: bool` is deleted. Update the module-level doc comment of
`src/policy/mod.rs`: keep the staging narrative (engine truthful, governance as overlay)
but replace the bool-only "Minimal preset" wording with the typed registry and the three
presets (fully_open, safe, restricted; built-in Minimal equals safe, per ADR-0019 "Safe
is today's Minimal"). Add `pub mod pattern;` next to the existing `pub mod redact;`.

### 3. The stage-2 initial key set

Add one `&'static str` name const per key (pattern: existing
`CONTENT_SECURITY_SECRETS_REDACT`), each with a short doc comment stating its semantics,
then register all seven in `KEYS` in exactly this order (the order of the shared format
doc section 3.4 table):

| Const | Key | Type | Constraint | fully_open | safe | restricted |
|---|---|---|---|---|---|---|
| `ENGINE_CONNECTION_FIRST_CALL_WAIT_MS` | `engine.connection.first_call_wait_ms` | Uint | `UintRange { min: 0, max: 60000 }` | `Uint(5000)` | `Uint(5000)` | `Uint(5000)` |
| `CONTENT_SECURITY_SECRETS_REDACT` | `content.security.secrets.redact` | Bool | `None` | `Bool(false)` | `Bool(true)` | `Bool(true)` |
| `CONTENT_SECURITY_SACRED_DOMAINS` | `content.security.sacred_domains` | StrList | `DomainPatternList` | `StrList(&[])` | `StrList(&[])` | `StrList(&[])` |
| `AUDIT_ENABLED` | `audit.enabled` | Bool | `None` | `Bool(false)` | `Bool(true)` | `Bool(true)` |
| `AUDIT_DESTINATION` | `audit.destination` | Enum | `EnumVariants(&["file", "stderr"])` | `Enum("file")` | `Enum("file")` | `Enum("file")` |
| `AUDIT_FILE_PATH` | `audit.file.path` | Str | `EmptyOrAbsolutePath` | `Str("")` | `Str("")` | `Str("")` |
| `GOVERNANCE_MODE` | `governance.mode` | Enum | `EnumVariants(&["observe", "enforce"])` | `Enum("observe")` | `Enum("enforce")` | `Enum("enforce")` |

Description strings, exactly (these surface in `config list` and the extension options
page, so they are part of the stable registry content):

- `engine.connection.first_call_wait_ms`:
  `Upper bound on the first-call wait for the extension handshake.`
- `content.security.secrets.redact` (unchanged from today):
  `Redact values of secret fields (password/OTP/payment) in read_page output.`
- `content.security.sacred_domains`:
  `User-authored never-touch domain patterns; always enforced, regardless of governance mode or manifest presence.`
- `audit.enabled`:
  `Record one audit line per tool call (the flight recorder).`
- `audit.destination`:
  `Where audit records are written. Takes effect on restart.`
- `audit.file.path`:
  `Audit file path; empty means the platform default location. Takes effect on restart.`
- `governance.mode`:
  `Default enforcement mode when the active manifest does not set one: observe records shadow denials, enforce blocks.`

Keep the existing rich doc comment on the `CONTENT_SECURITY_SECRETS_REDACT` const. For
the six new consts, a 1-3 line doc comment each, drawn from the semantics bullets in
shared format doc section 3.4 (note in the sacred-domains comment that matching semantics
land with the matcher task; only pattern syntax is validated here).

The restricted preset equals safe for every stage-2 key by design; it is registered now
so the preset name is stable (shared format doc 3.4).

### 4. Validation: `KeyDef::parse_value`

```rust
/// Validate a JSON value against this key's type and constraint, returning the owned
/// typed value on success. This is the single validation path for every write surface
/// (config files, CLI, native-messaging settings; those land in later tasks).
pub fn parse_value(&self, value: &serde_json::Value) -> Result<ConfigValue, ConfigValueError>
```

Rules, per the base type from `self.key_type()` (shared format doc 3.2):

- `Bool`: `value.as_bool()` or `Err(ConfigValueError::ExpectedBool)`. Strings, numbers,
  null, everything else fails.
- `Uint`: read the bounds from the key's `UintRange` constraint (if the constraint is
  not `UintRange`, use `min = 0, max = u64::MAX`; a registry-integrity test forbids that
  case for registered keys). `value.as_u64()` must return `Some(v)` with
  `min <= v && v <= max`, else `Err(ConfigValueError::ExpectedUint { min, max })`.
  Note `as_u64()` already rejects negatives, fractions, and exponent-form floats
  (serde_json parses `5e3` and `5.0` as f64, for which `as_u64()` is `None`), which is
  exactly the "integer, no sign, no fraction, no exponent" rule.
- `Enum`: the constraint must be `EnumVariants(variants)`. `value.as_str()` must equal
  one of `variants`, case-sensitively. Any other value (wrong string, non-string) fails
  with `Err(ConfigValueError::ExpectedVariant { variants })`.
- `Str`: `value.as_str()` or `Err(ConfigValueError::ExpectedString)`. If the constraint
  is `EmptyOrAbsolutePath`, additionally require the string to be empty OR
  `std::path::Path::new(s).is_absolute()`, else
  `Err(ConfigValueError::ExpectedAbsolutePath)`.
- `StrList`: `value` must be a JSON array whose every element is a string, else
  `Err(ConfigValueError::ExpectedStringList)`. Duplicate elements (exact string
  equality) fail with `Err(ConfigValueError::DuplicateEntry(<the duplicated string>))`.
  Element order is preserved in the returned `StrList`. If the constraint is
  `DomainPatternList`, every element must satisfy
  `crate::policy::pattern::is_valid_pattern`, else
  `Err(ConfigValueError::InvalidDomainPattern(<the offending element>))`. Report the
  first failure encountered scanning left to right. An empty array is valid.

Any JSON shape not covered above (null, object, mixed array) is invalid for every key;
this falls out of the rules as written.

The error type, with `thiserror` and EXACTLY these display messages (section 9.2 of the
shared format doc pins the uint wording: "expected an integer between 0 and 60000"):

```rust
/// A value failed validation against a [`KeyDef`]. Display strings are user-facing:
/// they appear verbatim in CLI errors and the native-messaging `invalid_value` message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigValueError {
    #[error("expected a boolean")]
    ExpectedBool,
    #[error("expected an integer between {min} and {max}")]
    ExpectedUint { min: u64, max: u64 },
    #[error("expected one of: {}", .variants.join(", "))]
    ExpectedVariant { variants: &'static [&'static str] },
    #[error("expected a string")]
    ExpectedString,
    #[error("expected an empty string or an absolute path")]
    ExpectedAbsolutePath,
    #[error("expected an array of strings")]
    ExpectedStringList,
    #[error("duplicate list entry: {0}")]
    DuplicateEntry(String),
    #[error("invalid domain pattern: {0}")]
    InvalidDomainPattern(String),
}
```

### 5. Domain pattern syntax: new file `src/policy/pattern.rs`

Create the file with a module doc comment explaining: this is the SYNTACTIC half of the
shared format doc section 5.1 pattern grammar, used to validate authored patterns (the
`content.security.sacred_domains` key now; manifest grant domains in a later task).
Matching SEMANTICS (host normalization, wildcard matching, the section 5.3 negative test
classes) belong to the matcher task and are NOT implemented here.

```rust
/// True when `pattern` is a syntactically valid domain pattern (shared format doc 5.1):
/// an exact host (`example.com`, `127.0.0.1`) or a single leading `*.` wildcard
/// (`*.example.com`). Lowercase ASCII only; IDN domains must be authored in punycode
/// (A-label) form. IPv6-literal patterns are not accepted by this syntactic check.
pub fn is_valid_pattern(pattern: &str) -> bool
```

Exact rules, applied in order:

1. The pattern must be non-empty and pure ASCII. Any non-ASCII byte is invalid.
2. If the pattern starts with `*.`, strip that prefix once. The remainder is validated as
   a host and must itself be non-empty. A `*` anywhere else (including a bare `*`, `*.`
   with empty remainder, `**.example.com` which leaves `*.example.com` containing `*`,
   or `foo.*.com`) is invalid.
3. The (remaining) host must be one or more labels separated by single `.` characters:
   no leading dot, no trailing dot, no empty label (`example..com` invalid).
4. Each label is 1 to 63 characters, each character one of `a-z`, `0-9`, or `-`, and the
   label neither starts nor ends with `-`. Uppercase ASCII letters are invalid (patterns
   are authored lowercase). This grammar rejects schemes (`https://...`), ports (`:`),
   paths (`/`), userinfo (`@`), and whitespace by construction, and naturally accepts
   IPv4 dotted literals such as `127.0.0.1` (digits are valid label characters).

Unit tests in the same file (inline `#[cfg(test)]`), covering at least:

- Valid: `example.com`, `*.example.com`, `localhost`, `127.0.0.1`, `a-b.example.com`,
  `xn--pple-43d.com`.
- Invalid: empty string, `*`, `*.`, `**.example.com`, `foo.*.com`, `Example.com`,
  `https://example.com`, `example.com/path`, `example.com:8443`, `user@example.com`,
  `.example.com`, `example.com.`, `example..com`, `-foo.example.com`,
  `foo-.example.com`, a 64-character label, and a non-ASCII pattern (for example
  `b\u{fc}cher.de` written with a Rust unicode escape in the test source so the source
  file itself stays ASCII).

### 6. Config growth and server wiring

Replace the single-field `Config` with owned typed fields for all seven keys. `Config`
LOSES `Copy` (it now holds `String` and `Vec<String>`); derive
`#[derive(Debug, Clone, PartialEq)]`.

```rust
pub struct Config {
    first_call_wait_ms: u64,
    secrets_redact: bool,
    sacred_domains: Vec<String>,
    audit_enabled: bool,
    audit_destination: String,
    audit_file_path: String,
    governance_mode: String,
}
```

Accessors, one per field, each with a doc comment naming its dotted key (keep the
existing `secrets_redact()` wording):

- `pub fn first_call_wait_ms(&self) -> u64`
- `pub fn secrets_redact(&self) -> bool`
- `pub fn sacred_domains(&self) -> &[String]`
- `pub fn audit_enabled(&self) -> bool`
- `pub fn audit_destination(&self) -> &str`
- `pub fn audit_file_path(&self) -> &str`
- `pub fn governance_mode(&self) -> &str`

Construction MUST read from the registry so `KEYS` stays the single source of truth:

- `pub fn from_preset(preset: Preset) -> Self` builds every field from
  `key_def(<CONST>).expect(...)` and `default_for(preset)`, extracting the typed value
  through small private helpers (one per `KeyValue` variant family, for example
  `fn preset_bool(key: &str, preset: Preset) -> bool`), each of which panics with a clear
  message if the registered default has the wrong variant. These panics are unreachable
  for a well-formed registry and every preset is exercised by unit tests, so drift is
  caught by `cargo test`, not at runtime in the field. Document that reasoning briefly.
- `pub fn minimal() -> Self` becomes `Self::from_preset(Preset::Safe)` (built-in Minimal
  equals safe). Keep its doc comment, updated.
- `Default` keeps delegating to `minimal()`.

Server wiring in `src/mcp/server.rs`:

- Change `handle_line` and `handle_tools_call` to take `config: &Config` instead of
  `config: Config` (the by-value passing only worked because `Config` was `Copy`). Fix
  the call sites. Post-T04, the `"tools/call"` arm spawns a task; there, clone first
  (`let config = config.clone();`) and pass `&config` inside the spawned future. Do the
  same for any other spawn that needs config.
- If `FIRST_CALL_WAIT_MS` exists in `src/mcp/server.rs` (T04 landed): delete the constant
  and its doc comment, and replace BOTH uses with the config value. In
  `handle_tools_call`, use `Duration::from_millis(config.first_call_wait_ms())`. In the
  `"initialize"` arm's warmup watcher, copy the value out before the spawn
  (`let wait_ms = config.first_call_wait_ms();` -- `u64` is `Copy`) and use
  `Duration::from_millis(wait_ms)` inside the spawned future. The T04 timeout message
  interpolates `FIRST_CALL_WAIT_MS / 1000`; change that to
  `config.first_call_wait_ms() / 1000` so the message stays truthful if the value ever
  differs from 5000. Behavior is byte-identical today because the safe default is 5000,
  the same number the constant held.
- If `FIRST_CALL_WAIT_MS` does NOT exist (T04 not landed yet): make no behavioral change
  to `src/mcp/server.rs` beyond the `&Config` signature change, and add a marked
  integration point comment near `PROTOCOL_VERSION`:
  `// INTEGRATION POINT (release-1 T04): the bounded first-call wait must read`
  `// config.first_call_wait_ms() (key engine.connection.first_call_wait_ms), not a constant.`

No other `src/mcp/server.rs` change. `dispatch::policy_check` / `dispatch::audit` calls,
the redact call, response shapes, and all JSON-RPC semantics stay untouched.

### 7. Unit tests (inline `#[cfg(test)]` in `src/policy/mod.rs`)

Registry integrity (extend or replace the two existing tests; keep their intent):

- `every_key_name_is_dotted_and_unique`: extend the existing test so that, in addition
  to dotted + unique + non-empty description, every dot-separated segment of every key
  name is non-empty and matches `[a-z0-9_]+` (check chars directly; do not add a regex
  crate).
- `every_key_defaults_share_one_type`: for every key, the three preset defaults have the
  same `KeyValue` variant (use `std::mem::discriminant`).
- `every_typed_key_declares_its_constraint`: every `Uint` key has a `UintRange`
  constraint with `min <= max`; every `Enum` key has `EnumVariants` with at least two
  variants, and each of its three defaults is a member of the variant list.
- `every_preset_default_parses_against_its_own_key`: for every key and every preset,
  `def.parse_value(&ConfigValue::from(def.default_for(preset)).to_json())` is `Ok`.
  This single test proves the registry, the conversion, and the validator agree.

Config construction:

- `minimal_config_matches_the_registry_defaults`: extend the existing test to assert all
  SEVEN accessors of `Config::minimal()` against the registry's safe defaults (looked up
  through `key_def` + `default_for(Preset::Safe)`, not against literals).
- `restricted_preset_equals_safe_for_stage_2`:
  `Config::from_preset(Preset::Restricted) == Config::from_preset(Preset::Safe)`.
- `fully_open_preset_opens_the_governed_defaults`: `from_preset(Preset::FullyOpen)` has
  `secrets_redact() == false`, `audit_enabled() == false`,
  `governance_mode() == "observe"`, and `first_call_wait_ms() == 5000`.
- `preset_names_round_trip`: `Preset::from_name(p.as_str()) == Some(p)` for all three
  presets, and `Preset::from_name("Safe")`, `from_name("full_open")`, `from_name("")`
  are `None`.

`parse_value`, exercising every type and every out-of-range case (use
`serde_json::json!` for inputs; use `serde_json::from_str::<serde_json::Value>("5e3")`
for the exponent case since `json!(5e3)` is already a float literal):

- Bool key (`content.security.secrets.redact`): `true` and `false` ok; `"true"`, `1`,
  `null`, `{}` all `Err(ExpectedBool)`.
- Uint key (`engine.connection.first_call_wait_ms`): `0`, `5000`, `60000` ok (both
  bounds inclusive); `60001`, `-1`, `1.5`, `"5000"`, and `5e3` all
  `Err(ExpectedUint { min: 0, max: 60000 })` (assert the exact error value at least
  once, and assert its display string is
  `expected an integer between 0 and 60000`).
- Enum key (`audit.destination`): `"file"` and `"stderr"` ok; `"syslog"`, `"File"`
  (case-sensitive), `1` all `Err(ExpectedVariant { .. })`; assert the display string is
  `expected one of: file, stderr`.
- Str key (`audit.file.path`): `""` ok; an absolute path ok (use
  `if cfg!(windows) { "C:\\logs\\audit.jsonl" } else { "/var/log/audit.jsonl" }`);
  `"logs/audit.jsonl"` is `Err(ExpectedAbsolutePath)`; `42` is `Err(ExpectedString)`.
- StrList key (`content.security.sacred_domains`): `[]` ok;
  `["example.com", "*.example.com"]` ok with order preserved in the returned
  `ConfigValue::StrList`; `["example.com", "example.com"]` is
  `Err(DuplicateEntry("example.com".into()))`; `["example.com", 3]` and a bare
  `"example.com"` are `Err(ExpectedStringList)`; `["EXAMPLE.com"]` and `["evil*.com"]`
  are `Err(InvalidDomainPattern(..))` naming the offending element.
- Null and object are invalid for every registered key: loop over `KEYS` asserting
  `parse_value` errors on `json!(null)` and `json!({})`.

Plus the `pattern.rs` tests from part 5. Do not add or change integration tests under
`tests/`; existing ones must pass unchanged.

## Constraints

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task touches no extension file at all.
3. All-open stays first-class and byte-identical: with no manifest and default config,
   every tool result is exactly what it is today. This task changes NO runtime behavior;
   the only wiring (first-call wait) substitutes a config value equal to the constant it
   replaces (5000). The extended `minimal_config_matches_the_registry_defaults` test and
   the untouched `tests/mcp_protocol.rs` are the guard.
4. ASCII only in ALL code and docs: no em-dashes, no arrows, no curly quotes, anywhere,
   including comments and test strings (use Rust `\u{..}` escapes where a test needs a
   non-ASCII input).
5. The engine is truthful: do not weaken any existing message. The T04 timeout message
   keeps interpolating the real wait value.
6. No new runtime dependencies and no `Cargo.toml` change. `serde_json` and `thiserror`
   already cover everything here. Do NOT add `sha2`, `uuid`, `chrono`, `regex`, or any
   schema crate; those belong to later tasks or to nobody.
7. Rust 2021 edition, `thiserror` for typed errors in library code, doc comments on all
   public items, module doc comments, `rustfmt` clean,
   `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline; no new files
   under `tests/`.
8. Do NOT copy code from other projects; implement from the described behavior.

Task-specific:

9. Use the shared format doc's names verbatim: key names, preset names ("fully_open",
   "safe", "restricted"), type names ("bool", "uint", "enum", "string", "string_list"),
   and the error message vocabulary. Do not invent alternatives.
10. `KEYS` remains the single static registry; `Config::from_preset` must read defaults
    from it, never restate literals.
11. Do not modify `src/dispatch.rs`, `src/policy/redact.rs`, `src/lib.rs`,
    `src/main.rs`, anything under `src/native/`, `src/install/`, `src/tools/`, or
    `extension/`.
12. Only pattern SYNTAX validation in this task. No host normalization, no URL parsing,
    no wildcard MATCHING, none of the section 5.3 negative test classes; those are the
    matcher task's.
13. Register exactly the seven keys of shared format doc section 3.4. Do not add
    speculative keys.

## Verification

1. `cargo fmt` then `cargo clippy --all-targets -- -D warnings` from the repo root:
   clean.
2. `cargo test` from the repo root: all tests pass, including every new unit test named
   in Required behavior part 7, the `src/policy/pattern.rs` tests,
   `tests/tool_schema_fidelity.rs` unchanged, `tests/mcp_protocol.rs` unchanged, and
   `tests/peer_death.rs`.
3. If `target/debug/browser-mcp.exe` is locked by a running session, rename it aside
   (for example `mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`)
   and rebuild.
4. Grep checks: `FIRST_CALL_WAIT_MS` no longer appears anywhere in `src/` (when T04 had
   landed); `minimal_default` no longer appears anywhere in `src/`.
5. Manual check (binary-only change; restart the MCP client to pick up the new binary;
   no extension reload needed): a normal session behaves exactly as before. `read_page`
   on a page with a password field still shows `[value redacted]` (safe default keeps
   redaction on), and with Chrome fully closed a tool call still fails after about 5
   seconds with the T04 timeout message.

## Out of scope

- File loading and layer resolution (G02): no reading of `config.json` or `policy.json`,
  no `%APPDATA%`/`dirs` paths, no layer precedence, no resolved-value triple
  (`value`/`source`/`locked`), no preset persistence. `Preset` is just the enum here.
- CLI surfaces (G03): no `config list`, `config set`, `config get`, no clap changes, no
  `src/main.rs` changes.
- Any manifest logic: no manifest structs, parsing, schema field, grants, content hash,
  identity block, or `--manifest` handling.
- Domain matching semantics: no URL/host normalization, no wildcard matcher, no
  section 5.3 negative test classes, no sacred-domain ENFORCEMENT (the key exists;
  nothing reads it yet).
- The audit subsystem (G06): the three `audit.*` keys exist; nothing reads them yet.
- Read/write classification, dispatch changes, enforcement, denials, shadow mode
  (G05/G14/G15 and the enforcement tasks). `src/dispatch.rs` stays a documented no-op.
- The native-messaging settings protocol (shared format doc section 9), the extension
  options page, and JSON Schema generation.
- Key renames and deprecation handling (shared format doc 3.1 mentions it for FUTURE
  renames; nothing is renamed here).
- Adding `sha2`, `uuid`, `chrono`, or any other dependency.
