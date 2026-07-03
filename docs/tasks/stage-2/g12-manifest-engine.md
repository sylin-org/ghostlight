# G12: Manifest parsing, validation, and loading

## Goal

Implement the manifest engine's front half: parse, validate, and load the schema-2
manifest format defined in `docs/tasks/stage-2/00-shared-format.md` section 4 (top-level
identity fields, grants, config entries with `level`, `mode` at manifest and grant
level), compute the SHA-256 content hash (section 4.2), resolve the manifest SOURCE
(auto-loaded org policy file, `--manifest` / `BROWSER_MCP_MANIFEST` user source, or none
= all-open), and feed manifest config entries into the G02 layer model so org entries
occupy the org layers. Validation errors are precise: they name the field, the reason,
and (for syntax and shape errors) the line and column. Ship three example manifests
under `examples/` in the reconciled format, and tests proving valid examples parse,
each invalid-field case errors precisely, and no-manifest stays all-open. This is the
start of ADR-0018 step 3 (the manifest engine); nothing is enforced yet.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file location, enum value, and rule in this task comes from it verbatim.
  Read it before writing any code. Load-bearing sections here: 1.2 (org policy file
  paths), 1.3 (user-supplied manifest and the selection rule), 2 (layer model), 3.4
  (registered keys), 4.1-4.5 (manifest format, hash, grants, config entries, all-open),
  5.1 (pattern grammar; SYNTAX only -- matching semantics belong to G13), 8 (the 13-tool
  list source), 10 items 3, 5, 6, 8 (what replaced the SPEC-era format).
- G02 (layered configuration registry growth): REQUIRED. G12 hands manifest config
  entries to G02's layer resolver and validates entry values against G02's typed
  registry (the section 3.4 key set with `KeyValue` / `KeyConstraint`-style typing). If
  `src/policy/mod.rs` still has only the single-key boolean seed registry described in
  Current behavior below, G02 has not landed: STOP and report; do not build a second
  registry or resolver inside this task.
- All release-1 tasks in `docs/tasks/release-1/` are assumed landed.
- NOT prerequisites: G05 (classification), G13 (grant evaluation), G14 (advertisement
  filtering), G15 (shadow mode). Those consume what G12 produces.

Because G02 (and possibly other stage-2 tasks) reshape `src/policy/` and
`src/mcp/server.rs` before G12 runs, the Current behavior section below records the
tree at authoring time. Do NOT trust it as the state you will edit. Re-read every file
named there and integrate with the code the prerequisites actually produced.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles are separate OS processes bridged by tokio-native named-pipe / UDS
IPC. Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance
layer: a separable overlay (ADR-0013; all-open stays first-class), landed in
observe-then-enforce order (ADR-0018), configured through a typed key registry with
layered precedence (ADR-0019), with the org policy experience of ADR-0020 (manifest
identity in every audit record, stable denial ids) built on top.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc is the reconciled single source. Concretely for this task: SPEC sections 4.1,
4.2, and Appendix A describe an OLDER manifest format (schema 1, `access:
"observe" | "mutate"`, `defaults` and `audit` blocks, `unlisted_domains`). That format
is superseded (shared format doc section 10, items 2, 3, 5, 6). Implement ONLY the
schema-2 format of shared format doc section 4. Do not add compatibility parsing for
schema 1; a schema-1 manifest must fail with a precise unsupported-schema error.

Key facts about the format you are implementing (shared format doc section 4):

- Top level: `schema` (integer, must be 2), `name` (string, required), `version`
  (string, required), `mode` (optional, `"observe" | "enforce"`), `identity` (optional
  object: `resolved_by`, `principal`, `groups`, `resolved_at`), `grants` (required
  array, may be empty), `config` (optional array of config entries).
- Grant: `id` (required, unique in the manifest), `domains` (required, at least one
  section-5.1 pattern), `access` (required, `"read" | "write" | "all"`), `tools` (array
  of tool names or null), `exclude_tools` (array of tool names), `description`
  (optional), `mode` (optional per-grant override, `"observe" | "enforce"`).
- Config entry: `{ "key": <registered dotted key>, "value": <typed value>,
  "level": "mandatory" | "recommended" }`.
- The content hash is COMPUTED by the binary from the canonical bytes, never stored in
  the manifest (section 4.2).
- No manifest = all-open, first-class (section 4.5). Presence of a manifest with
  grants changes nothing in THIS task: enforcement is G13/G15.

## Current behavior

All facts verified against the working tree at authoring time.

- `src/policy/` contains exactly two files. `mod.rs` (104 lines) holds the seed
  registry: `KeyDef { key, description, minimal_default: bool }`, the `KEYS` table with
  one entry (`content.security.secrets.redact`), and `Config` with one field and
  `Config::minimal()`. `redact.rs` is the read_page redaction overlay. There is no
  manifest type, no parser, no loader, no layer resolver. (G02 will have grown this
  before you start; integrate with what it built.)
- `src/main.rs` defines the `--manifest` flag on `Cli` (lines 32-36:
  `manifest: Option<String>`, `value_name = "SOURCE"`, doc text says "Absent = all-open").
  `run_server(manifest: Option<String>, debug_on: bool)` (line 230) receives it,
  logs it in the startup `tracing::info!` (lines 231-235), and never parses it. The
  `BROWSER_MCP_MANIFEST` environment variable is not read anywhere under `src/` (only
  docs mention it).
- `src/mcp/server.rs` (155 lines): `run` (line 22) builds `let config =
  Config::default();` (line 28) and threads it through `handle_line` (line 55) to
  `handle_tools_call` (line 116). `run` takes only `browser`; no manifest reaches the
  server loop. `main.rs` calls `browser_mcp::mcp::server::run(browser)` at line 254.
- `src/dispatch.rs` is the documented no-op seam: `policy_check` (lines 23-25) always
  returns `PolicyDecision::Allow`; `audit` (line 30) does nothing. G12 does not touch
  this file.
- `examples/` does NOT exist. CLAUDE.md and SPEC Appendix A describe planned examples
  in the superseded schema-1 format; nothing is on disk.
- `tests/` contains `mcp_protocol.rs` (drives the real binary with no manifest and
  asserts `tools/list` equals the parsed fixture exactly -- the all-open byte-identity
  guard), `tool_schema_fidelity.rs` (sacred schema guard), and `peer_death.rs`. There is
  no `tests/manifest_validation.rs`.
- `Cargo.toml` dependencies: `tokio`, `serde` (derive), `serde_json` (with
  `preserve_order`), `clap`, `tracing`, `tracing-subscriber`, `thiserror`, `anyhow`,
  `dirs` (plus Windows-only `winreg`, `windows-sys`). There is NO `sha2`, `uuid`, or
  `chrono`.
- `src/lib.rs` declares `pub mod policy;`, so new public policy modules are reachable
  from integration tests as `browser_mcp::policy::...`.
- `src/mcp/tools.rs` embeds the sacred fixture:
  `pub const TOOLS_JSON: &str = include_str!("schemas/tools.json");`. The 13 advertised
  tool names, in fixture order: `tabs_context_mcp`, `tabs_create_mcp`, `navigate`,
  `computer`, `find`, `form_input`, `get_page_text`, `javascript_tool`,
  `read_console_messages`, `read_network_requests`, `read_page`, `resize_window`,
  `update_plan`.

## Required behavior

### 1. New modules

Create `src/policy/manifest.rs` (format types, validation, content hash) and
`src/policy/source.rs` (source-string grammar, platform paths, selection rule, load
orchestration). Declare both in `src/policy/mod.rs` (`pub mod manifest;` and
`pub mod source;`). Module-level doc comments state their role: manifest.rs is the
schema-2 manifest format per shared format doc section 4 (ADR-0018 step 3); source.rs
resolves WHERE the active manifest comes from per shared format doc sections 1.2-1.3.
Keep each file focused; validation tables and tests may push manifest.rs toward the
project's 800-line ceiling, so prefer moving pure helpers (for example pattern syntax
checking) into small private functions rather than inlining everything in one giant
function.

### 2. Types (`src/policy/manifest.rs`)

Define with serde derive, `#[serde(deny_unknown_fields)]` on every struct, and
`#[serde(rename_all = "lowercase")]` on every enum. Doc comments on all public items.

```rust
pub struct Manifest {
    pub schema: u32,
    pub name: String,
    pub version: String,
    pub mode: Option<Mode>,
    pub identity: Option<Identity>,
    pub grants: Vec<Grant>,
    #[serde(default)]
    pub config: Vec<ConfigEntry>,
    #[serde(skip)]
    pub hash: String, // computed by the loader (section 4.2), never authored
}

pub struct Identity {
    pub resolved_by: Option<String>,
    pub principal: Option<String>,
    pub groups: Option<Vec<String>>,
    pub resolved_at: Option<String>,
}

pub struct Grant {
    pub id: String,
    pub domains: Vec<String>,
    pub access: Access,
    pub tools: Option<Vec<String>>,
    pub exclude_tools: Option<Vec<String>>,
    pub description: Option<String>,
    pub mode: Option<Mode>,
}

pub enum Access { Read, Write, All }

pub enum Mode { Observe, Enforce }

pub struct ConfigEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub level: Level,
}

pub enum Level { Mandatory, Recommended }
```

Adjustments to this sketch:

- If G02 already defined an observe/enforce mode type (for the `governance.mode`
  registry key) or a level type, REUSE it instead of defining a duplicate. One enum per
  concept in `src/policy/`.
- `Identity` fields are all optional and informational (type-checked when present;
  `resolved_by` is a free string, NOT validated against the SPEC 4.2 enum -- the
  reconciled format keeps it informational). Unknown fields inside `identity` are still
  rejected by `deny_unknown_fields`.
- G14's prompt names this `Grant` type with exactly these fields; keep the names.

### 3. Parsing and validation pipeline

One public entry point in manifest.rs:

```rust
/// Parse and validate manifest JSON text (shared format doc section 4) and compute
/// its content hash (section 4.2). `source_label` names the origin (a file path or
/// `env://VAR`) for error messages.
pub fn parse_manifest(text: &str, source_label: &str) -> Result<Manifest, ManifestError>
```

Pipeline, in this exact order so every failure class gets its most precise error:

1. BOM strip: remove one leading UTF-8 BOM (`\u{FEFF}`) if present. All later steps,
   including the hash, operate on the stripped text.
2. Syntax: parse to `serde_json::Value`. On failure, return a syntax error carrying
   serde_json's line and column and its message.
3. Schema version: the root must be a JSON object with an integer `schema` field equal
   to `2`. Missing `schema`, non-integer `schema`, or any other value fails with a
   message naming the found value and stating that only schema 2 is supported. This
   check runs BEFORE shape validation so a schema-1 (SPEC-era) manifest fails with
   "unsupported schema", not a confusing unknown-field error.
4. Shape: typed deserialize `Manifest` from the STRING (not from the Value;
   deserializing from the string keeps line/column in serde errors). With
   `deny_unknown_fields`, typos and superseded blocks (`defaults`, `audit`, an authored
   `hash`) fail here with serde's message, which names the field and the line/column.
   Wrap it in a shape-error variant.
5. Semantic validation (field-path errors; no line numbers available at this stage,
   so the path must be exact, in the style `grants[1].domains[0]`):
   - `name` must be non-empty; `version` must be non-empty.
   - Grant `id` must be non-empty; duplicate grant ids across the manifest are an
     error naming the duplicated id.
   - `domains` must have at least one element; every element must pass pattern SYNTAX
     validation (section 4 below), with errors naming the offending pattern and the
     specific rule it broke.
   - `tools` (when a non-null array) and `exclude_tools` (when present) are mutually
     exclusive: both present as non-null arrays is an error naming both fields.
     `"tools": null` alongside `exclude_tools` is legal (null is the default). Empty
     arrays are legal (a lint concern for the later `policy explain` task, not an
     error here).
   - Every name in `tools` / `exclude_tools` must be one of the 13 advertised tool
     names. Derive the valid set by parsing `crate::mcp::tools::TOOLS_JSON`
     (`tools[*].name`); do NOT hardcode a second copy of the list. Unknown names are
     an error naming the bad name and its field path. Sub-action names (for example
     `left_click`) are NOT tool names and must fail this check.
   - Every config entry `key` must be registered in the G02 registry; unknown keys
     are an error naming the key. Every `value` must satisfy the key's type and
     constraints (section 3.2 of the shared format doc) using G02's validation;
     violations name the key and the constraint (for example "expected an integer
     between 0 and 60000").
6. Hash: re-serialize the step-2 `Value` with `serde_json::to_string` (compact; the
   already-enabled `preserve_order` feature keeps authored key order), take the SHA-256
   of the UTF-8 bytes, render as 64 lowercase hex characters, store in
   `manifest.hash`. This makes the hash insensitive to whitespace, line endings, and
   BOM, and sensitive to content and key order (shared format doc section 4.2).

`ManifestError` is a `thiserror` enum in manifest.rs. Suggested variants: `Syntax
{ line, column, message }`, `UnsupportedSchema { found: String }`, `Shape { message }`
(serde's text already contains field and position), and `Field { path: String,
reason: String }`. source.rs adds its own error variants (or a small separate enum) for
I/O and source-grammar failures: file read errors (with the path), missing or empty
environment variable (with the variable name), unsupported scheme. Every error's
`Display` output must let a human fix the manifest without reading Rust code.

### 4. Domain pattern SYNTAX validation

G12 validates pattern syntax only (shared format doc section 5.1). Matching semantics,
normalization, and the section 5.3 negative test classes belong to G13; do not
implement a matcher here.

A pattern string is valid when ALL of these hold:

- non-empty, and contains no `/`, `:`, `@`, or whitespace (rejects schemes, ports,
  paths, userinfo);
- `*` appears either not at all, or exactly once as the literal prefix `*.` followed
  by at least one character (rejects bare `*`, `*.` alone, `foo.*.com`, `*.*.com`,
  `ex*mple.com`);
- after stripping an optional leading `*.`, the remainder does not start or end with
  `.` and contains no empty label (rejects `.example.com`, `example.com.`,
  `example..com`).

Case and non-ASCII are NOT load errors: hosts and patterns are compared after ASCII
lowercasing at match time (section 5.2, G13's job), and non-ASCII patterns draw a
warning from the later `policy explain` task, not a load failure. Implement this as a
small pure function (for example `fn validate_pattern(p: &str) -> Result<(), String>`
returning the broken rule as text) so unit tests hit it directly.

### 5. Source resolution (`src/policy/source.rs`)

Source-string grammar (applies to the `--manifest` value, and to the value of
`BROWSER_MCP_MANIFEST` when the flag is absent; the flag wins when both are set):

- starts with `env://`: the remainder is an environment variable name; that variable's
  value is the manifest JSON text itself (inline JSON). Missing or empty variable is a
  load error naming the variable.
- starts with `file://`: strip the prefix; if the remainder matches `/<drive letter>:`
  (for example `/C:/policy.json`), strip the leading slash too (Windows file-URL
  convenience); the result is a filesystem path.
- starts with `managed://`: error stating managed storage delivery is not supported in
  this release (deferred; shared format doc section 10 item 8).
- contains `://` with any other scheme: error naming the scheme.
- otherwise: the whole string is a filesystem path.

Org policy file path (shared format doc section 1.2), by `cfg(target_os)`:

- Windows: `%ProgramData%\browser-mcp\policy.json` (read the `ProgramData` environment
  variable; fall back to `C:\ProgramData` when unset).
- macOS: `/Library/Application Support/browser-mcp/policy.json`.
- Linux and other unix: `/etc/browser-mcp/policy.json`.

Selection rule (shared format doc section 1.3), implemented as a pure function over
explicit inputs (org file text present or not, user source text present or not) so
tests never touch real system paths or ambient environment:

1. If the org policy file exists, it is the active manifest. A user-supplied manifest,
   if also given, is STILL parsed and validated (its errors are still fatal), its
   grants are ignored with a `tracing::warn!` on stderr, and its config entries apply
   at the user layer. Record the fact on the load result (see below) so the audit
   task can note it in the first audit record of the session.
2. Else, the user-supplied manifest (if any) is active.
3. Else, no manifest: all-open.

Load result type (public, doc-commented; adjust naming only if G02 already introduced
an equivalent session-policy struct):

```rust
pub struct LoadedPolicy {
    /// The active manifest, or None for all-open.
    pub manifest: Option<Manifest>,
    /// Where the active manifest came from, when there is one.
    pub origin: Option<ManifestOrigin>,
    /// True when an org policy file displaced a user-supplied manifest's grants
    /// (shared format doc 1.3 rule 1). The audit task notes this in the first record.
    pub user_manifest_ignored: bool,
}

pub enum ManifestOrigin { OrgPolicyFile, UserFile, UserEnv }
```

Failure policy: fail closed and truthfully. A source that is SELECTED but cannot be
read, parsed, or validated is a fatal startup error: `run_server` returns the error
before serving a single JSON-RPC line, so the process exits non-zero with the precise
message on stderr. Absence of a manifest is normal and is all-open; presence of a
broken one is never silently ignored (an org policy that fails open is worse than a
crash).

### 6. Layer integration (G02)

Feed config entries into G02's layer resolver per the shared format doc section 2:

- Org policy file entries: `"level": "mandatory"` populates the org-mandatory layer
  (precedence 1, locked); `"level": "recommended"` populates the org-recommended layer
  (precedence 3).
- User-supplied manifest entries: BOTH levels apply at the user layer (precedence 2,
  never locked); an entry with `"level": "mandatory"` is downgraded with a
  `tracing::warn!` naming the key (shared format doc section 1.3).
- Use G02's resolver API exactly as G02 built it; do not construct a parallel
  resolution path. The resolved-value triple (value, source, locked) semantics are
  G02's; G12 only supplies the org and user-manifest layers.

### 7. Startup wiring

In `src/main.rs` `run_server`, before starting the JSON-RPC loop (plain synchronous
I/O, before or outside `rt.block_on`):

1. Resolve the user source string: the `manifest` flag argument, else
   `BROWSER_MCP_MANIFEST` from the environment.
2. Run the source selection and loading above; on error, return it (main already
   returns `anyhow::Result`, so the message reaches stderr and the exit code is
   non-zero). Add context naming the source (path or `env://VAR`).
3. Log the outcome truthfully, once, via `tracing::info!`: with a manifest, its
   `name`, `version`, `hash`, manifest-level `mode` (or "unset"), and origin; without
   one, a plain "no manifest: all-open" line. Update the existing startup log line
   (lines 231-235) rather than adding a second one that still calls the raw flag
   string "manifest".
4. Pass the `LoadedPolicy` into `browser_mcp::mcp::server::run` (extend its signature)
   so it lives at the same scope where `config` is resolved (server.rs `run`, line 28
   today). G14 and G13 read the grants from there; G12 itself only (a) feeds the layer
   resolver and (b) holds the manifest for those later tasks. Do not change
   `tools_list_result`, `handle_tools_call` behavior, or `src/dispatch.rs`.

Keys resolved from manifest config entries that have no consumer yet (for example
`audit.enabled` before the audit task lands) resolve normally in the registry and
simply have no runtime effect; do not build their consumers here.

### 8. Example manifests (`examples/`)

Create the `examples/` directory with exactly these three files, byte-for-byte as
given below (2-space indent, LF line endings, trailing newline). They replace the
schema-1 examples of SPEC Appendix A. Note the deliberate rename: the SPEC's
"developer unrestricted" example cannot exist in the reconciled format because
unrestricted IS manifest absence (all-open, first-class, ADR-0013) and the pattern
grammar has no match-everything wildcard; its replacement `developer-observe.json` is
the flight-recorder configuration (empty grants in observe mode: everything executes,
would-be denials become shadow_deny records once the audit and shadow tasks land).

`examples/enterprise-healthcare.json`:

```json
{
  "schema": 2,
  "name": "enterprise-healthcare",
  "version": "2026.07.1",
  "mode": "observe",
  "identity": {
    "resolved_by": "managed_config",
    "principal": "GEISINGER\\jdoe",
    "groups": ["Dept-EA", "App-ServiceNow-Admin", "App-Epic-ClinicalRead"],
    "resolved_at": "2026-07-01T08:00:00Z"
  },
  "grants": [
    {
      "id": "servicenow",
      "domains": ["servicenow.geisinger.org"],
      "access": "all",
      "description": "ServiceNow incident and change management"
    },
    {
      "id": "epic-restricted",
      "domains": ["epic.geisinger.org", "mychart.geisinger.org"],
      "access": "all",
      "exclude_tools": ["javascript_tool"],
      "description": "EHR automation without arbitrary JS execution",
      "mode": "enforce"
    },
    {
      "id": "research",
      "domains": ["*.gartner.com", "*.forrester.com", "*.ieee.org", "scholar.google.com", "learn.microsoft.com"],
      "access": "read",
      "description": "External research resources"
    },
    {
      "id": "internal-docs",
      "domains": ["confluence.geisinger.org", "sharepoint.geisinger.org"],
      "access": "read",
      "description": "Internal documentation"
    }
  ],
  "config": [
    { "key": "audit.enabled", "value": true, "level": "mandatory" },
    { "key": "audit.destination", "value": "file", "level": "mandatory" },
    { "key": "content.security.secrets.redact", "value": true, "level": "recommended" }
  ]
}
```

`examples/developer-observe.json`:

```json
{
  "schema": 2,
  "name": "developer-observe",
  "version": "2026.07.1",
  "mode": "observe",
  "identity": {
    "resolved_by": "local_file",
    "principal": "local-user",
    "groups": [],
    "resolved_at": "2026-07-01T14:00:00Z"
  },
  "grants": [],
  "config": [
    { "key": "audit.enabled", "value": true, "level": "recommended" },
    { "key": "audit.destination", "value": "stderr", "level": "recommended" }
  ]
}
```

`examples/qa-staging.json`:

```json
{
  "schema": 2,
  "name": "qa-staging",
  "version": "2026.07.1",
  "mode": "enforce",
  "identity": {
    "resolved_by": "environment",
    "principal": "ci-runner",
    "groups": ["QA-Automation"],
    "resolved_at": "2026-07-01T10:00:00Z"
  },
  "grants": [
    {
      "id": "staging",
      "domains": ["*.staging.geisinger.org"],
      "access": "all",
      "description": "Full automation on staging environment"
    },
    {
      "id": "production-readonly",
      "domains": ["*.geisinger.org"],
      "access": "read",
      "description": "Read-only verification on production"
    }
  ],
  "config": [
    { "key": "audit.enabled", "value": true, "level": "mandatory" },
    { "key": "audit.destination", "value": "file", "level": "mandatory" },
    { "key": "audit.file.path", "value": "/var/log/browser-mcp/qa-audit.jsonl", "level": "mandatory" }
  ]
}
```

All config keys used above must already be registered by G02 (`audit.enabled`,
`audit.destination`, `audit.file.path`, `content.security.secrets.redact`; shared
format doc section 3.4). If any is missing from the registry, G02 is incomplete: stop
and report rather than registering keys ad hoc here.

### 9. Tests

Inline unit tests (`#[cfg(test)]`) in manifest.rs and source.rs for the pure pieces;
a new integration test file `tests/manifest_validation.rs` (the name CLAUDE.md
reserves for this) for the example files and the validation matrix, using the public
`browser_mcp::policy::manifest` / `browser_mcp::policy::source` API. No test may read
real platform paths (`ProgramData`, `/etc`) or mutate ambient environment variables
that another test reads; drive the pure functions with explicit inputs.

Required coverage, grouped:

Valid inputs:
- Each of the three `examples/*.json` files parses via `parse_manifest` (locate them
  with `env!("CARGO_MANIFEST_DIR")`). Assert per file: `schema == 2`, the expected
  `name`, grant count, and that `hash` is 64 chars of `[0-9a-f]`.
- A minimal manifest (`schema`, `name`, `version`, `grants: []`) parses; `mode`,
  `identity`, and `config` default correctly.
- `"tools": null` together with `exclude_tools` present is legal.

Invalid-field matrix (one test per case or a table-driven test; each case asserts the
error names the offending field or pattern):
- missing `schema`; non-integer `schema`; `schema: 1` (message says only 2 is
  supported);
- missing `name`; empty `name`; missing `version`;
- unknown top-level field (use `"defaults"` -- the SPEC-era block -- and an authored
  `"hash"` field; both must be rejected);
- `mode: "audit"` (invalid enum value);
- missing `grants`; `grants` not an array;
- grant missing `id`; duplicate grant ids;
- grant missing `domains`; `domains: []`;
- invalid patterns, each rejected with the pattern named: `https://example.com`,
  `example.com:8443`, `example.com/path`, `user@example.com`, `ex*mple.com`, `*`,
  `*.`, `foo.*.com`, `.example.com`, `example.com.`, `example..com`, `""`;
- `access` missing; `access: "mutate"` (the superseded vocabulary; error lists the
  allowed values read/write/all);
- `tools: ["upload_image"]` (unknown tool); `exclude_tools: ["left_click"]`
  (sub-action, not a tool); `tools` and `exclude_tools` both non-null arrays;
- grant `mode: "shadow"` (invalid enum value);
- config entry with unregistered key; config entry value of the wrong type (for
  example `{ "key": "audit.enabled", "value": "yes", ... }`); config entry missing
  `level`; `level: "optional"`;
- `identity.groups` as a string instead of an array; unknown field inside `identity`.

Hash (section 4.2 pins):
- The same manifest with LF vs CRLF line endings, with and without a leading BOM, and
  with different indentation produces the SAME hash.
- Reordering two top-level keys produces a DIFFERENT hash.
- Changing one character of a value produces a different hash.

Source grammar and selection:
- `env://MY_VAR` extracts `MY_VAR`; `file:///etc/x.json` yields `/etc/x.json`;
  `file:///C:/x.json` yields `C:/x.json`; bare `/etc/x.json` passes through;
  `managed://` and `weird://x` are precise errors.
- Selection (pure function): org present + user present -> org active,
  `user_manifest_ignored == true`; org absent + user present -> user active; both
  absent -> `manifest == None`, all-open.

All-open invariant:
- Loading with no org file and no user source yields `LoadedPolicy { manifest: None,
  origin: None, user_manifest_ignored: false }`.
- The existing `tests/mcp_protocol.rs` byte-identity test passes UNCHANGED: the binary
  with no manifest still advertises the full fixture verbatim and answers tool calls
  exactly as today.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. This task reads `TOOLS_JSON`
   (for tool-name validation) and never writes anywhere near it. No advertisement
   changes here (that is G14).
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task changes no file under `extension/`.
3. All-open stays first-class: with no manifest and default config, behavior is
   byte-identical to today (SPEC 5.3 STEP 0 short-circuits to Allow; the dispatch seam
   stays untouched). Loading machinery must be pure plumbing when no manifest exists;
   the invariant is tested (section 9, all-open invariant).
4. ASCII only in ALL code and docs, including this task's tests, error messages, and
   the example JSON files: no em-dashes, arrows, or curly quotes.
5. The engine is truthful: the startup log states plainly whether a manifest is active
   and which one (name, version, hash, mode, origin); a broken selected manifest is a
   fatal, precisely-worded startup error, never a silent fallback to all-open;
   ignored user grants and downgraded mandatory entries are warned about by name.
6. New dependency: `sha2 = "0.10"` ONLY, for the SHA-256 content hash. The shared
   format doc's crate note sanctions it (hand-rolling cryptographic hashing is not
   acceptable). Do NOT add `uuid`, `chrono`, `url`, `jsonschema`, `schemars`, or
   anything else in this task.
7. Rust 2021 edition; `thiserror` for the error types (library code); doc comments on
   every public item; `cargo fmt` clean; `cargo clippy --all-targets -- -D warnings`
   clean. Unit tests inline; integration tests in `tests/manifest_validation.rs`.
8. Do NOT copy code from other projects (including `reference/`); implement from the
   behavior described here.
9. Use the shared format doc's names verbatim: `schema`, `name`, `version`, `mode`,
   `identity`, `grants`, `config`, `id`, `domains`, `access` (`read`/`write`/`all`),
   `tools`, `exclude_tools`, `description`, `key`, `value`, `level`
   (`mandatory`/`recommended`), `observe`/`enforce`. Never the SPEC-era vocabulary
   (`observe`/`mutate` as ACCESS values, `defaults`, `unlisted_domains`, manifest
   `audit` block).
10. If this prompt and `docs/tasks/stage-2/00-shared-format.md` ever disagree, stop
    and report; do not guess.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including every test in section 9,
   `tests/tool_schema_fidelity.rs` unchanged, and `tests/mcp_protocol.rs` unchanged.
4. Manual checks:
   - `cargo run -- --manifest file://examples/enterprise-healthcare.json` (then type
     nothing and close stdin, or Ctrl+C): the startup log names the manifest
     `enterprise-healthcare`, version `2026.07.1`, a 64-hex hash, mode `observe`,
     origin user file.
   - Point `--manifest` at a file containing `{"schema":1}`: non-zero exit, error
     says only schema 2 is supported.
   - Point it at a file with a bad domain pattern: non-zero exit, error names the
     pattern and the grant path.
   - Run with no manifest: startup log says all-open; an MCP client session behaves
     exactly as before this task.
5. Non-ASCII scan on everything you touched, for example
   `rg -n "[^\x00-\x7F]" src/policy/manifest.rs src/policy/source.rs examples/ tests/manifest_validation.rs`;
   there must be no hits.
6. `git status`: changes limited to `src/policy/mod.rs` (module declarations and any
   G02-resolver touchpoints), the two new policy modules, `src/main.rs`,
   `src/mcp/server.rs` (signature threading only), `Cargo.toml` / `Cargo.lock`
   (`sha2`), `examples/` (three new files), and `tests/manifest_validation.rs`.
   `src/mcp/schemas/tools.json`, `src/dispatch.rs`, and `extension/` show no diff.

Build note: binary changes need an MCP client restart to observe. If
`target/debug/browser-mcp.exe` is locked by a running session, rename it aside
(`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild.
No extension reload is needed for this task.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Grant EVALUATION (G13): no domain MATCHING against URLs or hosts, no URL parsing,
  no normalization (case, port, trailing dot, IP literals, punycode), none of the
  section 5.3 negative test classes, no per-call grant resolution, no "first matching
  grant wins" logic. G12 validates pattern SYNTAX only.
- Tool advertisement filtering (G14): `tools_list_result` in `src/mcp/server.rs`
  stays exactly as it is; the full 13-tool surface remains advertised with or without
  a manifest.
- Enforcement of any kind: `src/dispatch.rs` is untouched; `PolicyDecision` keeps its
  single `Allow` variant; no denials, denial ids, denial messages, or holds. A loaded
  manifest changes NOTHING about which calls execute in this task.
- Shadow mode / effective-mode resolution (G15): parse and store `mode` fields; do
  not compute the per-grant > manifest > `governance.mode` precedence or emit
  `shadow_deny` anywhere.
- The audit subsystem: no audit records, no `src/audit/`, no JSONL writing. The
  `user_manifest_ignored` flag and `manifest.hash` are exposed FOR the audit task,
  not consumed here.
- `managed://` / `chrome.storage.managed` manifest delivery: rejected with a clear
  error; deferred beyond stage 2 (shared format doc section 10 item 8).
- `http://` / `https://` manifest sources (SPEC section 10 exclusion stands).
- Mid-session manifest reload, file watching, or re-advertisement: the manifest is
  loaded once at startup.
- The user config file (`config.json`), preset selection, and the layer resolver
  itself: G02 owns them; G12 only feeds manifest-derived entries in.
- `policy explain`, `policy simulate`, generated JSON Schema, doctor governance
  output, and the native-messaging settings protocol (`get_status` / `get_config` /
  `set_config_key`): other stage-2 tasks.
- Editing `docs/SPEC.md` (Appendix A stays stale for now; the amendment is tracked in
  the shared format doc's "SPEC updates needed" list) or any ADR.
- Manifest signing or cryptographic trust in the org policy file path: file ACLs plus
  the deployment channel are the guard (shared format doc section 1.2); do not add
  signature checks.
