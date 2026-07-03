# G04: Generated JSON Schema and key reference docs

## Goal

Implement ADR-0020 commitment 1 for the configuration registry: two new CLI subcommands,
`browser-mcp config schema` (prints a JSON Schema draft 2020-12 document describing the
user configuration file) and `browser-mcp config docs` (prints a markdown key reference),
both generated at runtime from the typed key registry in `src/policy/mod.rs`, hand-rolled
with `serde_json` (no schema crate). Golden tests pin both outputs byte-for-byte so any
registry drift (new key, edited description, changed constraint) fails the build until
the goldens are regenerated deliberately. The schema, the editor experience, and the docs
therefore cannot drift apart, and none of them can drift from the code.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` (sections 1.1, 2, 3.1, 3.2, 3.3, 3.4, and the
  `get_config` descriptions in 9.2). Read it before writing any code; its field names,
  file formats, and key set are authoritative.
- The registry growth task (the earlier stage-2 task that grows `KeyDef` per shared
  format doc section 3.3 and registers the full seven-key set of section 3.4 with typed
  per-preset defaults and constraints). PRECONDITION CHECK before you start: open
  `src/policy/mod.rs` and confirm the `KEYS` registry contains all seven section-3.4
  keys with typed values and constraints. If it still shows the original single-key seed
  (`KeyDef { key, description, minimal_default: bool }` with only
  `content.security.secrets.redact`), the prerequisite has not landed: STOP and report;
  do not implement registry growth yourself.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed.
- G05 (classification), G06 (audit), and the layered-config loading task are NOT
  prerequisites; this task reads only the compiled-in registry.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance layer per
ADR-0013 (separable overlay; all-open stays first-class), ADR-0018 (observe-then-enforce
sequencing), ADR-0019 (layered configuration and typed key registry), and ADR-0020 (org
policy experience). ADR-0020 commitment 1 says: each release publishes a JSON Schema
generated from the key registry (hand-rolled generation, no new dependencies), and
reference documentation for keys is generated from the same registry, so the schema, the
editor experience, and the docs cannot drift apart. This task builds the generators and
the CLI surface for the CONFIG side of that commitment.

Scope boundary that matters: the shared format doc defines two file families. The USER
CONFIG FILE (shared format doc section 1.1: optional `preset`, optional flat `config`
map) is what this task's schema describes. The ORG POLICY FILE (section 1.2) is a
MANIFEST (section 4: `schema`, `name`, `version`, `mode`, `identity`, `grants`,
`config`); its schema is task G12, not this one. This task exposes its per-key value
schema generator as a public function precisely so G12 can reuse it for manifest
`config` entries without duplicating the type mapping.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc is the reconciled single source for formats and names; use its names, never
improvised ones.

Key files for this task:

- `src/policy/mod.rs` -- the governance module and typed key registry (`KeyDef`, `KEYS`,
  `Config`). Your new module is declared here. You read the registry; you do not change
  any registered key, description, default, or constraint.
- `src/main.rs` -- the clap CLI. Subcommands `Install`, `Uninstall`, `Doctor`, `Status`
  exist today; installer subcommands run synchronously with no tokio runtime. Your two
  subcommands follow that synchronous pattern.
- `src/lib.rs` -- already declares `pub mod policy;` (line 23), so integration tests can
  call your generators through the `browser_mcp` library crate.
- `Cargo.toml` -- `serde_json` is already present WITH the `preserve_order` feature
  (line 12). That feature makes `serde_json::Value` objects keep insertion order, which
  is what makes your generated output deterministic. Do not add any dependency.
- `src/mcp/schemas/tools.json` -- the SACRED tool schema fixture. Completely unrelated
  to this task; never touched.

## Current behavior

Snapshot as of authoring (prerequisite tasks may have grown `src/policy/mod.rs` by the
time you run; the CLI and test facts below still hold):

- There is no schema generation, no `config` subcommand, and no markdown generation
  anywhere in the codebase. `browser-mcp --help` lists exactly four subcommands:
  `install`, `uninstall`, `doctor`, `status`.
- `src/main.rs` defines `enum Command { Install(InstallArgs), Uninstall(UninstallArgs),
  Doctor(DoctorArgs), Status(StatusArgs) }` and dispatches in `fn main()` via a match on
  `Cli::parse()`. `install`/`uninstall`/`doctor` call synchronous functions in
  `browser_mcp::install`; no async runtime is built for them.
- `src/policy/mod.rs` at authoring time holds the registry seed:
  `KeyDef { key, description, minimal_default: bool }`, a `KEYS` table with the single
  entry `content.security.secrets.redact` (description: "Redact values of secret fields
  (password/OTP/payment) in read_page output."), and `Config` with `Config::minimal()`.
  The registry growth task replaces `minimal_default: bool` with typed values plus
  constraints and registers all seven section-3.4 keys. Adapt to the field and type
  names that task chose; the OUTPUT you must generate is fully specified below and does
  not depend on those internal names.
- `tests/` contains exactly three files: `mcp_protocol.rs`, `peer_death.rs`,
  `tool_schema_fidelity.rs`. There is no `tests/golden/` directory.
- The repository has no `.gitattributes` file at the root or anywhere else.

## Required behavior

### 1. New module `src/policy/schema.rs`

Create `src/policy/schema.rs` and declare it in `src/policy/mod.rs` by adding
`pub mod schema;` alongside the existing module declarations (after `pub mod redact;`
and any module lines earlier stage-2 tasks added). That one-line addition is the ONLY
change to `mod.rs`.

Module-level doc comment: this module generates the user-config-file JSON Schema and the
markdown key reference from the typed key registry (ADR-0020 commitment 1); the registry
is the single source of truth, both outputs are pinned by golden tests, and the per-key
value schema function is reused by the manifest schema (task G12).

The module is pure: no I/O, no CLI parsing, no file paths. It exposes exactly four
public functions:

```rust
/// JSON Schema (draft 2020-12) fragment validating one key's VALUE, derived from the
/// key's registered type, constraints, description, and built-in Minimal default.
/// Reused by the manifest schema generator (task G12) for manifest config entries.
pub fn key_value_schema(def: &KeyDef) -> serde_json::Value

/// The complete JSON Schema (draft 2020-12) document for the user configuration file
/// (shared format doc section 1.1).
pub fn config_file_schema() -> serde_json::Value

/// `config_file_schema()` pretty-printed (serde_json 2-space style) plus exactly one
/// trailing LF. This exact string is what `browser-mcp config schema` prints and what
/// the golden test pins.
pub fn render_config_schema() -> String

/// The markdown key reference generated from the registry, LF line endings, exactly one
/// trailing LF. This exact string is what `browser-mcp config docs` prints and what the
/// golden test pins.
pub fn render_key_reference() -> String
```

(If the registry growth task renamed `KeyDef`, use its name; the signatures otherwise
stand.)

### 2. Per-key value schema (`key_value_schema`)

Map the section-3.2 type system to JSON Schema. Every generated object lists its members
in exactly this insertion order: `description`, then `type`, then constraint fields,
then `default`. `description` is the key's registered description VERBATIM (never
edited, appended to, or truncated). `default` is the key's built-in Minimal default
(which equals the `safe` preset default per shared format doc section 2).

| Registry type | Generated value schema |
|---|---|
| bool | `{"description": d, "type": "boolean", "default": <minimal>}` |
| uint | `{"description": d, "type": "integer", "minimum": <min>, "maximum": <max>, "default": <minimal>}` |
| enum | `{"description": d, "type": "string", "enum": [<variants in declared order>], "default": <minimal>}` |
| string | `{"description": d, "type": "string", "default": <minimal>}` |
| string list | `{"description": d, "type": "array", "items": {"type": "string"}, "uniqueItems": true, "default": [<minimal elements>]}` |

Rules:

- `uniqueItems: true` encodes the section-3.2 rule that duplicate list elements are
  rejected.
- Do NOT invent a regex `pattern` for domain-pattern lists
  (`content.security.sacred_domains`). The binary's loader validates patterns
  authoritatively; a wrong or partial regex in a published schema would be a trust
  defect. The items schema stays `{"type": "string"}`.
- No other annotation keywords: no `examples`, no `deprecated`, no `$comment`.

### 3. Document schema (`config_file_schema`)

The document describes the user configuration file of shared format doc section 1.1
exactly: both top-level fields optional, unknown fields flagged. Structure, with member
insertion order exactly as shown:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "browser-mcp user configuration file",
  "description": "User-level configuration for browser-mcp. Both fields are optional; an absent file means no user layer.",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "preset": {
      "description": "Preset supplying the preset-default layer. When absent, the built-in Minimal defaults (equal to safe) apply.",
      "type": "string",
      "enum": ["fully_open", "safe", "restricted"]
    },
    "config": {
      "description": "Flat map of dotted key name to value. Each entry sets the user layer for that key.",
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "<one property per registered key, in KEYS registry order, each generated by key_value_schema>": {}
      }
    }
  }
}
```

Rules:

- The three literal description strings and the title above are pinned verbatim; copy
  them exactly (ASCII, no trailing spaces).
- There is NO `$id` member. The project rename is pending; do not invent a URL.
- There is NO `required` array anywhere (every field is optional).
- `properties.config.properties` has exactly one member per `KEYS` entry, keyed by the
  dotted key name, in registry declaration order. `preserve_order` keeps that order in
  the serialized output.
- The `preset` enum lists the three preset names in the fixed order `fully_open`,
  `safe`, `restricted` (shared format doc section 1.1).

`render_config_schema()` is `serde_json::to_string_pretty` of this value plus one
trailing `\n`. The string must contain no `\r` and no non-ASCII byte.

### 4. Markdown key reference (`render_key_reference`)

Exact format, pinned by the golden test. LF line endings only; one trailing LF; no
trailing spaces on any line. The file opens with this header (verbatim):

```
# Configuration key reference

Generated from the typed key registry in src/policy/mod.rs by `browser-mcp config docs`.
Do not edit by hand; change the registry and regenerate.

Layer resolution: org-mandatory > user > org-recommended > preset default > built-in
Minimal. The built-in Minimal defaults equal the `safe` preset.
```

Then, for each key in `KEYS` registry order, one section separated from what precedes it
by a single blank line:

```
## `<dotted key name>`

<the key's registered description, verbatim, on one line>

- Type: <type word>
- Constraints: <constraints phrase>
- Default (fully_open): <json>
- Default (safe, = built-in Minimal): <json>
- Default (restricted): <json>
```

Rules:

- `<type word>` is one of exactly: `bool`, `uint`, `enum`, `string`, `string list`
  (the section-3.2 vocabulary).
- `<constraints phrase>` by type:
  - bool: `none`
  - uint: `integer between <min> and <max>` (matching the section-9.2 invalid_value
    wording, e.g. `integer between 0 and 60000`)
  - enum: `one of: <variants joined by ", ">` (e.g. `one of: file, stderr`)
  - string: `none`
  - string list without a domain-pattern constraint: `unique string elements`
  - string list with the domain-pattern constraint: `unique string elements; each a
    valid domain pattern`
- Each `<json>` is the preset's default rendered by `serde_json::to_string` (compact):
  booleans as `true`/`false`, numbers bare, strings quoted (the empty string renders as
  `""`), lists as `[]` or `["a","b"]`.

### 5. CLI subcommands in `src/main.rs`

If an earlier stage-2 task already added a `Config` subcommand to `enum Command`
(e.g. for `config list | get | set`), ADD two variants to its existing subcommand enum.
If none exists (the authoring-time state), create it:

```rust
/// Inspect the configuration registry (generated schema and docs).
Config(ConfigArgs),
```

```rust
#[derive(Debug, Args)]
struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Print the JSON Schema (draft 2020-12) for the user configuration file.
    Schema,
    /// Print the markdown key reference generated from the key registry.
    Docs,
}
```

Dispatch in `fn main()`'s existing match, synchronously (no tokio runtime, same as the
installer subcommands):

- `config schema` runs
  `print!("{}", browser_mcp::policy::schema::render_config_schema());`
- `config docs` runs
  `print!("{}", browser_mcp::policy::schema::render_key_reference());`

`print!`, not `println!`: the rendered strings already end with exactly one LF. Nothing
else goes to stdout (tracing writes to stderr, so `init_tracing` needs no change). Both
commands exit 0. Intended usage: `browser-mcp config schema > browser-mcp.config.schema.json`.

### 6. Golden files and tests

Golden files live in a new directory `tests/golden/`:

- `tests/golden/config-schema.json` -- the exact output of `render_config_schema()`.
- `tests/golden/config-keys.md` -- the exact output of `render_key_reference()`.
- `tests/golden/.gitattributes` -- one line, `* text eol=lf`, so git never converts the
  goldens to CRLF on Windows checkouts (the repo has no root `.gitattributes`; this
  scoped file is the whole fix).

Bootstrap procedure (do it in this order):

1. Implement the module and CLI.
2. Generate the goldens from the implementation:
   `cargo run --quiet -- config schema > tests/golden/config-schema.json` and
   `cargo run --quiet -- config docs > tests/golden/config-keys.md`.
3. Open both files and verify BY HAND against sections 2-4 above: member order, pinned
   strings, all seven registry keys present, constraint phrases, defaults. Golden tests
   pin whatever is committed, so this review is the moment correctness is established.
4. Commit the goldens with the code.

New integration test file `tests/config_schema_golden.rs`, with a file-level doc comment
stating the intent: any registry change must fail here until the goldens are regenerated
deliberately, which is how schema/docs/code drift is caught (ADR-0020 commitment 1).
Required tests, by name and assertion:

1. `generated_schema_matches_the_golden_file`:
   `browser_mcp::policy::schema::render_config_schema()` equals
   `include_str!("golden/config-schema.json").replace("\r\n", "\n")`. The `replace` is a
   defense against a CRLF checkout only; comment it as such.
2. `generated_key_reference_matches_the_golden_file`: same pattern for
   `render_key_reference()` against `golden/config-keys.md`.
3. `schema_covers_the_registry_exactly`: parse `render_config_schema()` with
   `serde_json`; assert `$schema` equals
   `"https://json-schema.org/draft/2020-12/schema"`; assert `additionalProperties` is
   `false` at the top level AND inside `properties.config`; assert the member set of
   `properties.config.properties` equals the set of key names in
   `browser_mcp::policy::KEYS`, both directions (no missing key, no stale property);
   assert every per-key property object has a nonempty string `description`.
4. `key_reference_covers_the_registry_exactly`: for every entry in `KEYS`, the rendered
   markdown contains the line `## \`<key>\``; and the markdown contains exactly
   `KEYS.len()` occurrences of the substring `\n## `.
5. `outputs_are_ascii_and_lf_only`: both rendered strings contain no `\r` and no byte
   above 0x7F.

Inline unit tests (`#[cfg(test)] mod tests` in `schema.rs`), written over the real
`KEYS` entries so they do not depend on the registry's internal field names:

1. `every_key_value_schema_has_description_type_and_default`: for each `KEYS` entry,
   `key_value_schema` returns an object containing a nonempty `description`, a `type`
   member whose value is one of `boolean`, `integer`, `string`, `array`, and a `default`
   member.
2. `uint_keys_carry_bounds_and_enum_keys_carry_variants`: every generated schema with
   `"type": "integer"` has numeric `minimum` and `maximum`; every generated schema with
   an `enum` member has a nonempty array of strings there.
3. `rendering_is_deterministic`: `render_config_schema()` equals itself across two
   calls, and `render_key_reference()` likewise.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. This task does not touch tool
   advertisement or the MCP protocol at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task changes no extension file.
3. All-open stays first-class: with no manifest and default config, runtime behavior is
   byte-identical to today. This task guarantees that trivially: it adds two synchronous
   CLI subcommands and a pure module; `src/dispatch.rs`, `src/mcp/server.rs`, and every
   tool path are untouched.
4. ASCII only in ALL code, comments, docs, generated output, and golden files: no
   em-dashes, arrows, or curly quotes. The generated strings are themselves tested for
   this (test 5 above).
5. The engine is truthful: the generated schema and docs must state exactly what the
   registry declares; never editorialize, soften, or extend a registered description.
   If a registered description looks wrong, STOP and report; do not fix it here.
6. No new dependencies of any kind: no `schemars`, `jsonschema`, `valico`, `askama`,
   `handlebars`, or any schema/template crate. Generation is hand-rolled with
   `serde_json` (already in Cargo.toml with `preserve_order`) and `std::fmt::Write` /
   `String::push_str` for the markdown. `Cargo.toml` shows no diff.
7. Rust 2021 edition; doc comments on the module and every public function; `cargo fmt`
   clean; `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline in
   `schema.rs`; golden tests in `tests/config_schema_golden.rs`.
8. Do NOT copy code from other projects; implement from the behavior described here.
9. Use the shared format doc's names verbatim: key names of section 3.4, type words of
   section 3.2, preset names `fully_open` / `safe` / `restricted`, the layer wording of
   section 2. Do not invent aliases.
10. Do not change any registered key, description, default, or constraint in
    `src/policy/mod.rs`; the one-line `pub mod schema;` declaration is the only edit
    there.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including the five golden/coverage tests in
   `tests/config_schema_golden.rs`, the three unit tests in `src/policy/schema.rs`,
   `tests/tool_schema_fidelity.rs` unchanged, and every pre-existing test.
4. `cargo run --quiet -- config schema` prints a JSON document that starts with
   `{` and contains `"$schema": "https://json-schema.org/draft/2020-12/schema"`;
   piping it through `serde_json` in the tests already proves it parses.
5. `cargo run --quiet -- config docs` prints markdown starting with the exact line
   `# Configuration key reference`.
6. Byte-compare CLI output to the goldens:
   `cargo run --quiet -- config schema | git diff --no-index - tests/golden/config-schema.json`
   reports no difference (and the same for `config docs` vs
   `tests/golden/config-keys.md`).
7. `git status` shows changes ONLY to: `src/policy/mod.rs` (one added line
   `pub mod schema;`), the new `src/policy/schema.rs`, `src/main.rs`, the new
   `tests/config_schema_golden.rs`, and the new `tests/golden/` directory
   (`config-schema.json`, `config-keys.md`, `.gitattributes`).
   `src/mcp/schemas/tools.json` and `Cargo.toml` show no diff.
8. Grep the new and touched files for non-ASCII bytes, for example
   `rg -n "[^\x00-\x7F]" src/policy/schema.rs tests/config_schema_golden.rs tests/golden/`;
   there must be none.

Build note: if `target/debug/browser-mcp.exe` is locked by a running MCP session, rename
it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and
rebuild. No extension reload is needed; no MCP client restart is needed since the server
role's runtime behavior does not change.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- The MANIFEST schema (task G12). Nothing here describes `grants`, `identity`, `mode`,
  `name`, `version`, `schema`, or manifest `config` entries with `level`. The org
  policy file (shared format doc section 1.2) is a manifest and is G12's job; this
  task's document schema covers the section-1.1 user configuration file only.
  `key_value_schema` is made public FOR G12's later reuse, but no manifest-shaped JSON
  is generated here.
- Publishing and CI wiring. No GitHub Actions workflow, no release artifact step, no
  script that writes the schema anywhere except stdout. The files under `tests/golden/`
  are test fixtures, not published artifacts; "each release publishes" (ADR-0020) is a
  packaging task.
- Editor integration beyond the schema document itself. No VS Code `json.schemas`
  settings, no schema-store registration, no `$id` URL, no YAML support, no
  language-server pragmas.
- `config list | get | set`, the layered resolution engine, file loading of the user
  config or org policy file, presets taking effect, lock handling: all belong to other
  stage-2 tasks. This task never reads or writes any config file; it renders the
  compiled-in registry.
- Validating a user's config file against the generated schema at runtime. The loader
  task does its own typed validation; the schema is an authoring-time aid.
- Registry changes: no new keys, no description edits, no constraint changes, no
  `Config` field changes.
- The native-messaging settings protocol (`get_config` and friends, shared format doc
  section 9) and any extension surface.
- Any change under `extension/`, `src/dispatch.rs`, `src/mcp/`, or `docs/` (the SPEC
  amendment list is tracked in the shared format doc's "SPEC updates needed" section
  and is a separate docs task).
- Denials, audit records, classification, enforcement, or anything that alters a tool
  call's behavior.
