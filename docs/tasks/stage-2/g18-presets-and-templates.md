# G18: Presets (Fully Open, Safe, Restricted) and policy init templates

## Goal

Add two starting-point commands, both pure UX on top of machinery that other stage-2
tasks build:

- `browser-mcp config preset <fully-open|safe|restricted>` records a named preset in the
  user config file (the layer-4 selection of shared-format section 2), after showing the
  user a plain-language diff of what will effectively change. A preset is a starting
  point the user can then edit key by key; it never overwrites the user's own per-key
  settings and never touches org-locked keys.
- `browser-mcp policy init --template <name>` writes a named example manifest from an
  embedded set (three schema-2 manifests, also committed under `examples/`) as a
  starting point for org admins.

This is ADR-0019 decision 3 ("Presets are UX, not machinery") and the ADR-0020
consequence that manifest templates ride the same schema as `policy init --template`
starting points.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file location, enum value, and preset default in this task comes from it
  verbatim. Read it before writing any code. Load-bearing sections here: 1.1 (user
  config file path and format), 1.2 (org policy file paths), 2 (layer model; "choosing a
  preset sets layer 4 only"), 2.1 (resolved triple: value, source, locked), 3.4 (the
  per-preset defaults table), and 4 (manifest format the templates must satisfy).
- The stage-2 configuration-registry task: it grows `KeyDef` to typed values with one
  default per preset (`fully_open`, `safe`, `restricted`), implements the five-layer
  resolver returning the resolved triple, implements reading and writing the user config
  file of shared-format 1.1, and adds the `config list | get | set` CLI family
  (ADR-0019 decision 5). G18 adds `config preset` to that family and calls its resolver
  and its file read/write helpers. If the `config` subcommand family or the layered
  resolver does not exist in the tree when you start, STOP and land that task first; do
  not build a second resolver or a second config-file writer inside G18.
- The stage-2 manifest-loading and validation task: it parses and validates schema-2
  manifests (shared-format section 4). G18's embedded templates MUST validate through
  that real validator in tests. If no manifest parser/validator exists in `src/policy/`
  when you start, STOP and land that task first; do not hand-roll a template validator.
- G16 (the plain-language renderer behind `policy explain` and the import preview,
  ADR-0020 commitment 2): OPTIONAL. If its renderer has landed, `config preset` uses it
  to render the diff. If it has not landed, print the simple before/after table defined
  below and leave the marked integration point comment. Do not block on G16.

Because the prerequisites reshape `src/policy/` and `src/main.rs` before G18 runs, the
"Current behavior" section below records the tree as it stands today. Do NOT trust it as
the state you will edit. Re-read every file named below before changing it, and
integrate against the code the prerequisites actually produced.

## Project context

Browser MCP is a governed browser automation system. A single Rust binary is BOTH the
MCP server (JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging
host; a thin Manifest V3 extension executes CDP commands. The chain is:

    MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser

The two binary roles run as separate OS processes bridged by tokio-native named-pipe (on
Windows) or Unix-domain-socket (elsewhere) IPC.

Stage 1 (docs/tasks/release-1/) hardened the engine. This is stage 2, the governance
layer: a separable overlay (ADR-0013; all-open stays first-class), landed
observe-then-enforce (ADR-0018), configured through one typed key registry with layered
precedence (ADR-0019), with an org policy experience of generated schema, explain,
simulate, shadow mode, and stable denial ids (ADR-0020).

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc `docs/tasks/stage-2/00-shared-format.md` is the reconciled single source for
formats and names. Concretely for this task: SPEC Appendix A (docs/SPEC.md line 571)
contains three example manifests in the OLD schema-1 format (`access` values `observe` /
`mutate`, top-level `defaults` and `audit` blocks, no `name` / `version` / `mode`). That
format is superseded by shared-format section 4. Do NOT copy Appendix A verbatim; the
schema-2 rewrites given below in Required behavior are the authoritative template
contents.

The layer model this task serves (shared-format section 2):

| Precedence | Layer | Written by |
|---|---|---|
| 1 (highest) | org-mandatory | org policy file entries with `"level": "mandatory"` |
| 2 | user | user config file `config` map |
| 3 | org-recommended | org policy file entries with `"level": "recommended"` |
| 4 | preset default | the preset named in the user config file |
| 5 (lowest) | built-in Minimal | compiled-in registry defaults |

Choosing a preset sets layer 4 ONLY. The user's own per-key edits (layer 2) sit above
the preset, and org-mandatory (layer 1) sits above everything. "Safe" equals the
built-in Minimal defaults, so layer 5 is always a complete floor.

The per-preset defaults for the stage-2 key set (shared-format 3.4, reproduced; the
registry is the single source of truth and the prerequisite task encodes this table):

| Key | fully_open | safe (= Minimal) | restricted |
|---|---|---|---|
| `engine.connection.first_call_wait_ms` | 5000 | 5000 | 5000 |
| `content.security.secrets.redact` | false | true | true |
| `content.security.sacred_domains` | `[]` | `[]` | `[]` |
| `audit.enabled` | false | true | true |
| `audit.destination` | `file` | `file` | `file` |
| `audit.file.path` | `""` | `""` | `""` |
| `governance.mode` | `observe` | `enforce` | `enforce` |

## Current behavior

All facts verified against the working tree at authoring time.

`src/main.rs` (281 lines):

- The `Command` enum (lines 43-53) has exactly four variants: `Install`, `Uninstall`,
  `Doctor`, `Status`. There is NO `config` and NO `policy` subcommand family yet; the
  configuration-registry prerequisite adds `config`. Arg structs follow a consistent
  pattern (`InstallArgs` lines 55-81, with `--dry-run` as the no-write convention) and
  convert into option structs via `From` impls.
- `run_server` (line 230) receives `manifest: Option<String>` from `--manifest`.

`src/policy/mod.rs` (103 lines) holds only the seed registry today:

- `KeyDef` (lines 25-33) with a single boolean `minimal_default`.
- One registered key, `content.security.secrets.redact` (lines 39-47).
- `Config` (lines 51-70) with one field and `Config::minimal()`.
- There is NO typed value model, NO per-preset defaults, NO layered resolver, NO user
  config file I/O, and NO manifest type. The prerequisites add all of these.

`examples/` does NOT exist. The repository layout in `CLAUDE.md` plans
`examples/enterprise-healthcare.json`, `examples/developer-unrestricted.json`, and
`examples/qa-staging.json`; G18 creates the directory and those three files in the
schema-2 format.

`docs/SPEC.md` Appendix A (line 571; A.1 at 573, A.2 at 626, A.3 at 652) holds the
schema-1 ancestors of these examples. They are format-superseded (see Project context).
Do not edit the SPEC in this task.

The installer (`src/install/merge.rs`) is the house precedent for config-file writes:
idempotent, value-level JSON merge, re-reading the file at apply time and preserving
everything it does not own. `config preset` follows the same discipline for the user
config file.

`Cargo.toml`: `tokio`, `serde`, `serde_json` (with `preserve_order`), `clap` (with
`derive`), `tracing`, `tracing-subscriber`, `thiserror`, `anyhow`, `dirs`. There is no
dev-dependencies section. G18 adds no dependency of any kind.

## Required behavior

G18 delivers: (1) the `config preset` subcommand with its diff-before-write flow, (2)
the three example manifests under `examples/` plus their embedded copies, and (3) the
`policy init --template` subcommand. Reuse the prerequisite modules; add only what is
missing. Suggested new code homes (adjust to the module layout the prerequisites
actually created): preset selection and diff logic in `src/policy/presets.rs`, template
embedding and `policy init` logic in `src/policy/templates.rs`, CLI arg structs in
`src/main.rs` following the existing `InstallArgs` pattern. Every public item gets a doc
comment; every new module gets a module-level doc comment.

### 1. `config preset <name>`

CLI shape: `browser-mcp config preset <PRESET> [--dry-run]`, added to the existing
`config` subcommand family.

- `PRESET` is a clap `ValueEnum` with the three variants rendering as `fully-open`,
  `safe`, `restricted` on the CLI. Also accept the underscore forms (`fully_open`) as
  aliases. The value STORED in the user config file is always the underscore form of
  shared-format 1.1: `"fully_open"`, `"safe"`, `"restricted"`.
- `--dry-run`: print the diff (step c below) and write nothing. Last line:
  `Dry run: nothing written.`

Behavior, in order:

a. Resolve the CURRENT effective state: run the layered resolver over all registered
   keys with the layers as they are now (org file if present, user config file with its
   current `preset` field and `config` map, built-in Minimal).
b. Resolve the CANDIDATE effective state: identical inputs except layer 4 is the newly
   selected preset.
c. Show the change before writing. If the G16 plain-language renderer has landed, pass
   it the two resolved states and print its rendering. Otherwise print the fallback
   table below, and leave this exact comment at the call site:
   `// G16 integration point: replace this table with the plain-language diff renderer when it lands.`
d. Unless `--dry-run`: write the preset to the user config file (rules below) and print
   `Preset '<cli-name>' saved.` on success.

Fallback diff table (exact format). Header first:

```
Preset change: <current> -> <new>
User config file: <path>
```

`<current>` is the current preset's cli-name, or `(none)` when the user config file is
absent or has no `preset` field. `<path>` is the resolved shared-format 1.1 path. Then
one two-space-indented line per NOTEWORTHY key, in registry order; values render as
compact JSON (`true`, `"file"`, `[]`, `5000`):

- Effective value changes (not locked):
  `  <key>: <before> -> <after>`
- Key is locked (resolved source is org-mandatory) AND the new preset's default differs
  from the locked effective value:
  `  <key>: <effective> (managed by your organization; preset does not affect this key)`
- Key has a user-layer entry (resolved source is user) AND the new preset's default
  differs from that user value:
  `  <key>: <effective> (kept: your explicit setting overrides the preset)`
- All other keys print nothing.

If no line qualifies, print exactly:
`  no effective values change.`

Rules for the write (step d):

- Write ONLY the `preset` field of the user config file (shared-format 1.1). NEVER copy
  preset values into the `config` map: that would promote layer-4 defaults into layer 2
  and break "user edits sit above the preset". This is the mechanical meaning of
  "locked keys are skipped, never overwritten": the command writes no per-key values at
  all, so nothing can be overwritten; the diff carries the notices.
- Read-modify-write, re-reading the file at apply time (the `src/install/merge.rs`
  discipline): parse the existing file if present, set `preset`, preserve the `config`
  map and any unrecognized fields exactly as authored (`preserve_order` keeps key
  order). Reuse the config-registry task's read/write helper if it has one; do not fork
  a second writer.
- File absent: create the parent directory (`fs::create_dir_all`) and write
  `{ "preset": "<underscore-name>" }`.
- File present but not valid JSON: print an error naming the path, write NOTHING (never
  clobber a corrupt file), exit nonzero.
- Serialize with `serde_json::to_string_pretty` plus a trailing newline (or the
  prerequisite helper's existing style if it already writes this file).
- Selecting `safe` still writes `"preset": "safe"` even though resolution is then
  identical to no preset: the explicit choice is recorded, and `config list` shows
  source `preset` rather than `builtin` for keys it supplies.

Exit codes: 0 on success and on `--dry-run`; nonzero on unknown preset (clap handles),
on a corrupt user config file, and on any I/O failure.

### 2. Example manifests under `examples/`

Create the directory `examples/` at the repo root with exactly three files. Contents
are given verbatim; write them byte-for-byte (ASCII, two-space indent, LF line endings,
one trailing newline). These are valid schema-2 manifests per shared-format section 4.
Manifests are strict JSON: there is NO comment syntax, and you MUST NOT extend the
manifest parser to accept comments. The "commented starting point" quality comes from
the grant `description` fields and the orientation text `policy init` prints.

`examples/enterprise-healthcare.json`:

```json
{
  "schema": 2,
  "name": "enterprise-healthcare",
  "version": "2026.07.0",
  "mode": "observe",
  "identity": {
    "resolved_by": "managed_config",
    "principal": "EXAMPLE\\jdoe",
    "groups": ["Dept-EA", "App-ServiceNow-Admin", "App-Epic-ClinicalRead"],
    "resolved_at": "2026-07-01T08:00:00Z"
  },
  "grants": [
    {
      "id": "servicenow",
      "domains": ["servicenow.example.org"],
      "access": "all",
      "description": "ServiceNow incident and change management. Full read and write automation."
    },
    {
      "id": "ehr-restricted",
      "domains": ["epic.example.org"],
      "access": "all",
      "exclude_tools": ["javascript_tool"],
      "mode": "enforce",
      "description": "EHR automation without arbitrary JS execution. Enforced immediately, even while the rest of this manifest observes."
    },
    {
      "id": "research",
      "domains": ["*.gartner.com", "*.forrester.com", "*.ieee.org", "scholar.google.com", "learn.microsoft.com"],
      "access": "read",
      "description": "Read-only external research resources."
    },
    {
      "id": "internal-docs",
      "domains": ["confluence.example.org", "sharepoint.example.org"],
      "access": "read",
      "description": "Read-only internal documentation."
    }
  ],
  "config": [
    { "key": "audit.enabled", "value": true, "level": "mandatory" },
    { "key": "audit.destination", "value": "file", "level": "mandatory" },
    { "key": "content.security.secrets.redact", "value": true, "level": "recommended" }
  ]
}
```

`examples/developer-unrestricted.json` (a config-only manifest: empty `grants` means no
domain restriction per shared-format 4.5, so this demonstrates audit-on with no
enforcement; `level` values are `recommended` so loading it as a user-supplied manifest
produces no downgrade warnings per shared-format 1.3):

```json
{
  "schema": 2,
  "name": "developer-unrestricted",
  "version": "2026.07.0",
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

`examples/qa-staging.json` (demonstrates first-match-wins grant ordering: the specific
staging grant precedes the broader production grant):

```json
{
  "schema": 2,
  "name": "qa-staging",
  "version": "2026.07.0",
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
      "domains": ["staging.example.org", "*.staging.example.org"],
      "access": "all",
      "description": "Full automation on the staging environment. Listed before the broader production grant: first match wins."
    },
    {
      "id": "production-readonly",
      "domains": ["example.org", "*.example.org"],
      "access": "read",
      "description": "Read-only verification on production. Broader pattern, so it comes last."
    }
  ],
  "config": [
    { "key": "audit.enabled", "value": true, "level": "mandatory" },
    { "key": "audit.destination", "value": "file", "level": "mandatory" }
  ]
}
```

Embed each file into the binary with `include_str!` (relative path from the embedding
module, e.g. `include_str!("../../examples/qa-staging.json")`), so the committed
example and the shipped template are the same bytes by construction. Template names are
the file stems: `enterprise-healthcare`, `developer-unrestricted`, `qa-staging`.

### 3. `policy init --template <name>`

CLI shape: `browser-mcp policy init --template <NAME> [--out <PATH>] [--force]`.

- If the prerequisites already created a `policy` subcommand family (for `policy
  explain` / `policy simulate`), add `init` to it. If none exists yet, create the
  `Policy` variant on `Command` in `src/main.rs` with `Init` as its first subcommand,
  following the existing arg-struct pattern.
- `--template <NAME>` is required. `NAME` must be one of the three template names.
  Unknown name: print an error listing the three valid names, exit nonzero, write
  nothing.
- `--out <PATH>` is optional; default is `policy.json` in the current working
  directory.
- If the output path already exists: print an error saying the file exists and that
  `--force` overwrites it, exit nonzero, write nothing. With `--force`, overwrite.
- On success, write the embedded template bytes EXACTLY (no re-serialization, no
  mutation) to the output path, then print this orientation block (exact text;
  substitute `<path>` with the written path and `<name>` with the template name):

```
Wrote <path> (template '<name>').

This file is a starting point. Edit the grants and config entries for your
organization, then deploy it with your management channel (GPO, Intune, Jamf)
to the org policy path for each platform:

  Windows  %ProgramData%\browser-mcp\policy.json
  macOS    /Library/Application Support/browser-mcp/policy.json
  Linux    /etc/browser-mcp/policy.json

For personal use, load it instead with:
  browser-mcp --manifest file:///absolute/path/to/policy.json

Manifests are strict JSON (no comments). Grant "description" fields carry the
explanatory text.
```

`policy init` writes only the file the user asked for. It never touches the user config
file, never touches the org policy paths, and never fetches anything from the network.

### 4. Tests

Inline `#[cfg(test)]` unit tests next to the code; integration tests in `tests/`. No
new dev-dependencies: build temp paths from
`std::env::temp_dir().join(format!("browser-mcp-g18-{}", std::process::id()))` and
clean up. At minimum:

1. Preset name mapping: CLI `fully-open` and alias `fully_open` both select the same
   variant; the stored file value is `"fully_open"`; same for the other two presets.
2. Writing to a missing user config file creates the parent directory and produces
   `{ "preset": "restricted" }` (plus nothing else).
3. Writing preserves the rest of the file: given an existing file with a `config` map
   and an unknown extra field, after `config preset` the `config` map and the unknown
   field are value-identical and only `preset` changed.
4. Corrupt file: given a file containing `not json`, the preset write fails, exits the
   command path with an error, and the file bytes are unchanged.
5. Diff rows (pure function test over resolved states): with an org-mandatory
   `audit.enabled: true` and a user-layer `content.security.secrets.redact: true`,
   switching from `safe` to `fully_open` yields a changed row for `governance.mode`
   (`"enforce" -> "observe"`), a locked notice for `audit.enabled` (fully_open default
   `false` differs from the locked `true`), and a kept notice for
   `content.security.secrets.redact` (fully_open default `false` differs from the user
   `true`). Keys whose values do not differ produce no row.
6. No-change case: from pristine defaults (no user file, no org file), selecting `safe`
   yields `no effective values change.` and still records `"preset": "safe"`.
7. Every embedded template validates through the real manifest loader/validator:
   schema 2 accepted, grant ids unique, `access` values legal, domain patterns legal,
   every `config` entry key registered with a type-valid value. Pin the qa-staging
   grant order: ids `["staging", "production-readonly"]` in that order.
8. Embedded bytes match disk: for each template, `include_str!` content equals the
   bytes of the corresponding `examples/*.json` file (this is true by construction;
   assert the file parses as JSON and re-serializes losslessly, or simply assert
   non-emptiness plus JSON validity of the embedded constant).
9. `policy init` integration (subprocess, temp cwd): running
   `policy init --template qa-staging` creates `policy.json` whose bytes equal the
   embedded template; a second run without `--force` exits nonzero and mentions
   `--force`; with `--force` it succeeds; `--template no-such-name` exits nonzero and
   the error output contains all three valid template names.
10. All-open invariant: with no user config file, no org file, and no manifest, the
    resolver yields the built-in Minimal value for every registered key, and the
    existing test suite (including `tests/tool_schema_fidelity.rs` and
    `tests/mcp_protocol.rs`) passes unchanged. G18's commands only run when invoked;
    default startup behavior is byte-identical to before.

## Constraints

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged. G18 changes no tool
   schema text and does not touch tool advertisement.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. G18 touches no extension file at all.
3. All-open stays first-class: with no manifest and default config, behavior is
   byte-identical to today (enforcement STEP 0 short-circuits to Allow). Presets and
   templates are opt-in commands; test 10 pins the invariant.
4. ASCII only in all code, docs, JSON, and printed text: no em-dashes, no Unicode
   arrows, no curly quotes, anywhere, including comments and the example manifests.
   The two-character ASCII sequence `->` in the diff table is fine.
5. The engine is truthful: the diff must state exactly what changes and what does not
   (locked and kept notices); never present a preset as protection it does not provide.
   The `enterprise-healthcare` template's `"mode": "observe"` is intentionally paired
   with the shadow-badge work of G15; do not change it to hide that pairing.
6. No new dependencies of any kind, including dev-dependencies. `serde_json` with
   `preserve_order`, `clap` with `derive`, and `dirs` already cover everything G18
   needs. JSON handling is `serde_json` only. No network access anywhere in this task.
7. Rust 2021 edition; `thiserror` for library error types; doc comments on every public
   item and module doc comments on new modules; `cargo fmt` clean;
   `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline, integration
   tests in `tests/`.
8. Do NOT copy code from the official Anthropic extension, the reference
   implementation, or any other project; implement from the behavior described here.

Task-specific:

9. `config preset` writes ONLY the `preset` field of the user config file. It never
   writes per-key values into the `config` map, never writes the org policy file, and
   never deletes user entries. Presets populate layer 4 only (shared-format section 2).
10. Locked keys are never written by any G18 code path; the diff carries the
    "managed by your organization" notice instead. The notice wording must contain
    exactly the phrase `managed by your organization` (it must agree with the
    shared-format 9.2 badge and the `locked` error message of shared-format section 9).
11. Templates are strict JSON. Do not add comment support to the manifest parser, do
    not write JSONC, and do not post-process the embedded bytes on write.
12. Reuse the prerequisite machinery: the layered resolver, the resolved triple, the
    user-config read/write helper, the manifest validator, and (if landed) the G16
    renderer. Do not fork a second resolver, writer, validator, or preset enum.
13. The three template names, their file names under `examples/`, and their `name`
    fields must agree exactly (`enterprise-healthcare`, `developer-unrestricted`,
    `qa-staging`).

## Verification

1. From the repo root: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test` are all clean. `tests/tool_schema_fidelity.rs` passes without any edit.
2. Rebuild the binary. If `target/debug/browser-mcp.exe` is locked by a running
   session, rename it aside first (for example `mv target/debug/browser-mcp.exe
   target/debug/browser-mcp.exe.old-1`) and rebuild. Binary changes require an MCP
   client restart to observe in a live session; no extension reload is needed (no
   extension change).
3. `browser-mcp config preset restricted --dry-run` prints the `Preset change:` header,
   the user config file path, either noteworthy key lines or
   `no effective values change.`, and ends with `Dry run: nothing written.`; the user
   config file is not created or modified.
4. `browser-mcp config preset fully-open` (no dry-run) prints the diff (expect at least
   `governance.mode`, `audit.enabled`, and `content.security.secrets.redact` rows
   against pristine safe defaults), then `Preset 'fully-open' saved.`. Inspect the user
   config file (on Windows `%APPDATA%\browser-mcp\config.json`): it contains
   `"preset": "fully_open"`; any pre-existing `config` entries are untouched. If
   `config list` exists, keys not otherwise set now report source `preset`.
5. With an org policy file present that locks `audit.enabled` to `true` (or with the
   equivalent unit-test fixture if you cannot write the admin path), `config preset
   fully-open` shows the locked notice for `audit.enabled` and the effective value
   stays `true`.
6. In an empty temp directory: `browser-mcp policy init --template qa-staging` creates
   `policy.json` byte-identical to `examples/qa-staging.json` and prints the
   orientation block. Running it again exits nonzero mentioning `--force`; with
   `--force` it succeeds. `--template bogus` exits nonzero and lists the three valid
   names.
7. Load the generated file through the real manifest path (for example
   `browser-mcp --manifest file:///<temp>/policy.json` or the loader's own test
   harness): it parses and validates with no errors and no warnings other than the
   expected user-supplied-manifest notes of shared-format 1.3.
8. All-open check: delete (or move aside) the user config file, ensure no org policy
   file and no `--manifest`, start the server, and confirm a normal `initialize` +
   `tools/list` + one tool call behave exactly as before this task.

## Out of scope

- A community template gallery, registry, or any shared-manifest distribution
  mechanism (future work per ADR-0020; the embedded set is the whole stage-2 surface).
- ANY network fetching: no template downloads, no URL sources for `policy init`, no
  update checks. The template set is compiled in.
- The layered resolver, the typed `KeyDef` growth, the user config file format itself,
  and the `config list | get | set` commands (configuration-registry task). G18 only
  adds `config preset` beside them.
- The G16 plain-language renderer itself, `policy explain`, `policy simulate`, and the
  generated JSON Schema (other stage-2 tasks). G18 only calls the renderer if it
  already exists, with the fallback table and the marked integration point otherwise.
- Manifest parsing, grant resolution, enforcement, denial formatting, audit records,
  and shadow mode (other stage-2 tasks). G18 only consumes the validator in tests.
- Writing to, creating, or managing the org policy file paths (ProgramData, /Library,
  /etc). `policy init` writes to a user-chosen path; deployment is the org channel's
  job (GPO, Intune, Jamf).
- Comment support, JSONC, YAML, or any non-JSON manifest format.
- Editing `docs/SPEC.md` (Appendix A stays as-is; the SPEC amendment list lives in
  shared-format section 10 and is a separate docs task).
- Any extension change, any change to `src/mcp/schemas/tools.json`, tool routing,
  dispatch, IPC, the installer, or the debug/observability subsystem.
- Preset deprecation or migration machinery, additional preset names, per-preset
  divergence for `restricted` (it equals `safe` for every stage-2 key; the name is
  registered now so it stays stable), or interactive confirmation prompts.
- New dependencies in `Cargo.toml`, including dev-dependencies.
