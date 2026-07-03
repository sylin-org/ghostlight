# G16: Policy explain (deterministic plain-language rendering)

## Goal

Add `browser-mcp policy explain <file>`: a CLI command that renders a policy manifest (or
a user configuration file) as deterministic plain-language sentences an administrator can
review and a user can trust. The rendering states: the policy's identity and version,
where agents may read and write (grants in match order), what is locked at which level,
what users may still change, the mode (enforce or shadow, stated bluntly), and what
happens on denial. Every sentence comes from the fixed template set in this prompt;
iteration order is deterministic; golden tests pin the full byte-for-byte output for the
committed example manifests. A rendering bug that misstates policy is a serious defect,
so the tests are the point of this task, not an afterthought. The renderer is exported as
a public library function so the future import-preview surface reuses the exact same
sentences: the sentence an admin reviews is the sentence a user sees.

This is ADR-0020 commitment 2.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file format, enum value, and pattern rule in this task comes from it
  verbatim. Read it before writing any code. Load-bearing sections here: 1.1 (user
  config file format), 2 (layer model), 3 (key registry, 3.4 key set and `governance.mode`
  precedence), 4 (manifest format, 4.2 content hash, 4.3 grants, 4.4 config entries,
  4.5 all-open), 5.1 (pattern grammar and the non-ASCII warning), 7 (denial format the
  explain text describes).
- The stage-2 manifest parsing + validation + content-hash task: it owns the parsed
  manifest type (`schema`, `name`, `version`, `mode`, `identity`, `grants`, `config`
  per shared format section 4.1), its validation errors (unknown schema, duplicate grant
  ids, unknown tool names, unknown config keys, invalid patterns), and the SHA-256
  content hash over canonical bytes (section 4.2). G16 calls that parser and that hash
  function; it must NOT implement a second parser or a second hash. If that machinery
  does not exist yet under `src/policy/`, stop and land the prerequisite first.
- The stage-2 configuration-registry task: it grows `KeyDef` in `src/policy/mod.rs` to
  typed values with constraints and per-preset defaults (shared format section 3.3) and
  registers the stage-2 key set (section 3.4). G16 reads key descriptions and the
  built-in default of `governance.mode` from that registry, and reuses its user-config
  file loader (section 1.1) if one exists.
- G15 (shadow enforcement), or whichever prerequisite added the `Mode` enum
  (`Observe` / `Enforce`) and the optional `mode` fields on the manifest and on each
  grant. Reuse that type; do not define a second mode enum.

Several prerequisites reshape `src/policy/` and `src/main.rs` before G16 runs. The
"Current behavior" section below records the tree as it stands at authoring time. Do NOT
trust it as the state you will edit: re-read every file named below and integrate against
the types the prerequisites actually produced.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles are separate OS processes bridged by tokio-native named-pipe / UDS
IPC. Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance
layer: a separable overlay (ADR-0013; all-open stays first-class), landed
observe-then-enforce (ADR-0018), configured through a typed key registry with layered
precedence (ADR-0019), with an org policy experience of generated schema, explain,
simulate, shadow mode, manifest identity, and structured denials (ADR-0020).

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc is the reconciled single source. Relevant here: SPEC Appendix A (line 571)
contains schema-1 example manifests with the OLD `access: observe | mutate` vocabulary
and the removed `defaults` / `audit` blocks. Those examples are superseded; the schema-2
examples authored by this task (below) replace them as the canonical examples. Do not
copy Appendix A shapes.

Why explain exists (ADR-0020): every harvested prior-art admin experience is authoring
blind (ADMX XML, deploy and pray). Explain makes the policy file previewable: an admin
reads exactly what the policy does before shipping it, in sentences generated from the
same parsed structures the engine enforces, so the preview cannot drift from behavior.
ADR-0020's consequences section names the risk this task must engineer against:
"`policy explain` is a trust surface. A rendering bug that misstates policy is a serious
defect; the renderer needs golden tests that pin sentences to decisions."

## Current behavior

All facts verified against the working tree at authoring time.

- `src/main.rs`: the `Command` enum (lines 43-53) has exactly four subcommands:
  `Install`, `Uninstall`, `Doctor`, `Status`. There is no `policy` subcommand. The
  dispatch `match Cli::parse()` is at lines 173-195. The existing `--manifest` flag
  (lines 32-35) is a server-role option and is unrelated to this task's positional file
  argument. `Install`/`Uninstall`/`Doctor` run synchronously with no tokio runtime;
  `policy explain` follows that pattern.
- `src/policy/` contains exactly `mod.rs` and `redact.rs` today. `mod.rs` (103 lines)
  holds the seed registry: `KeyDef { key, description, minimal_default }` (lines 25-33),
  the one-entry `KEYS` table for `content.security.secrets.redact` (lines 43-47), and
  `Config` with `Config::minimal()` (lines 51-70). There is no manifest type, no parser,
  no hash function, no `Mode` enum, and no explain module. Grep for `explain` under
  `src/` finds nothing. The prerequisites add the missing machinery.
- `tests/` contains `mcp_protocol.rs`, `peer_death.rs`, `tool_schema_fidelity.rs`. There
  is no `tests/fixtures/` directory. `tests/mcp_protocol.rs` line 22 shows the pattern
  for spawning the built binary in an integration test:
  `Command::new(env!("CARGO_BIN_EXE_browser-mcp"))`.
- There is no `examples/` directory at the repository root (CLAUDE.md's layout names one;
  it has never been created).
- `Cargo.toml` dependencies: `tokio`, `serde`, `serde_json` (with `preserve_order`),
  `clap` 4 (derive), `tracing`, `tracing-subscriber`, `thiserror` 2, `anyhow`, `dirs`
  (plus Windows-only `winreg` / `windows-sys`). The shared format doc notes `sha2` and
  friends arrive with earlier stage-2 tasks; G16 adds no dependency of its own.

## Required behavior

### 1. New module `src/policy/explain.rs`

Create `src/policy/explain.rs` and declare it in `src/policy/mod.rs` with
`pub mod explain;`. Module doc comment: this is the deterministic plain-language policy
renderer (ADR-0020 commitment 2); it is a trust surface whose sentences are pinned by
golden tests; the exported functions are reused by the future import-preview surface so
admin preview and user preview can never differ.

Public API (adapt parameter types to the ones the prerequisites actually defined; the
shapes below are the contract):

```rust
/// Render a parsed manifest as the fixed-template plain-language explanation.
/// `hash` is the 64-lowercase-hex content hash (shared format 4.2) of the source bytes.
/// Pure: no I/O, no clock, no randomness, no platform lookups. Deterministic:
/// identical input yields byte-identical output on every platform.
pub fn explain_manifest(manifest: &Manifest, hash: &str) -> String

/// Render a parsed user configuration file (shared format 1.1).
/// `warnings` are the load warnings (unknown key, invalid value) in file order.
pub fn explain_user_config(file: &UserConfigFile, warnings: &[String]) -> String

/// Load a file, detect its kind, and render it. This is the function the CLI calls.
/// Detection: read the bytes, strip a UTF-8 BOM if present, parse as JSON; a top-level
/// object containing a "schema" member is a manifest (validated by the prerequisite
/// parser; validation failures are errors); anything else is treated as a user config
/// file (shared format 1.1; unknown keys and invalid values become warnings, not
/// errors). Errors use a thiserror enum (ExplainError), not anyhow.
pub fn explain_file(path: &Path) -> Result<String, ExplainError>
```

If the configuration-registry prerequisite does not expose a user-config loader with
warnings, validate the `config` map entries against the registry locally inside
`explain_file` (unknown key or type/constraint-invalid value produces one warning string
each, file order) -- but never duplicate MANIFEST parsing or hashing.

`explain_file` must not read the live org policy file, the live user config file, any
environment variable, or any platform path. It explains exactly one file in isolation,
over the compiled-in registry defaults. This is what makes the goldens machine
independent and the preview trustworthy.

### 2. Output structure

The rendered string is a sequence of blocks joined by exactly one blank line (`\n\n`),
ending with exactly one trailing `\n`. Line separator is always `\n` (LF), never `\r\n`,
on every platform. All output is ASCII (the input may contain non-ASCII, e.g. a
principal name; input values are echoed as-is, and non-ASCII DOMAIN PATTERNS additionally
draw a warning, per shared format 5.1).

Manifest rendering emits these blocks in this exact order:

1. Header
2. Identity
3. Mode
4. Grants
5. Settings
6. Denial
7. Warnings (only when at least one warning exists)

### 3. Fixed templates, block by block (manifest)

Angle brackets are substitutions; everything else is literal. Do not paraphrase,
reorder, or "improve" any sentence.

Block 1, header (two lines):

```
Policy '<name>', version <version>.
Content hash: <hash>.
```

Block 2, identity (one line):

- `identity` present:
  `Prepared for '<principal>', resolved by <resolved_by>.` and, only when `groups` is
  non-empty, append on the same line: ` Groups: <groups comma-joined with ", " in
  manifest order>.`
- `identity` absent:
  `No identity block: this policy does not name a principal.`

Block 3, mode (one line). First resolve the manifest-level effective mode and its
source, in this order (this mirrors shared format 3.4 for a file explained in
isolation):

- (a) the manifest `mode` field, when present: no suffix;
- (b) else a manifest `config` entry for `governance.mode` with `"level": "mandatory"`:
  suffix ` This mode is locked by the policy.`;
- (c) else a manifest `config` entry for `governance.mode` with
  `"level": "recommended"`: suffix ` This mode is a default the user may change.`;
- (d) else the registry's built-in default for `governance.mode` (enforce): suffix
  ` This policy sets no mode; the built-in default applies.`

Base sentence by resolved mode, with the suffix (if any) appended:

- enforce: `Mode: enforce. Calls the grants below do not permit are blocked.`
- observe: `Mode: observe (shadow). Nothing is blocked by this policy: would-deny events
  are recorded to the audit log and the calls proceed. Observation is not protection.`

(Written as one output line; the wrapping above is only for this document.)

Block 4, grants. When `grants` is non-empty:

```
Where agents may read and write, in match order (the first matching domain wins):
(a pattern like 'example.com' matches only that exact host; '*.example.com' matches its subdomains and never example.com itself)
  1. <grant line>
  2. <grant line>
<unmatched line>
```

Each grant line is prefixed `  <n>. ` (two spaces, 1-based index in manifest order, dot,
one space) and is composed of these sentences in this exact order, single-space
separated, each present only under its condition:

1. Access sentence (always), where `<domains>` is the grant's patterns comma-joined with
   `", "` in manifest order:
   - `access: "read"`:
     `Read-only on <domains>: agents may read pages but not act on them.`
   - `access: "write"`:
     `Write-only on <domains>: agents may act on pages but not read them (write does not include read).`
   - `access: "all"`:
     `Full access on <domains>: agents may read and act.`
2. Tool restriction sentence (when present; `tools` and `exclude_tools` are mutually
   exclusive per shared format 4.3), tool names comma-joined with `", "` in manifest
   order:
   - `tools` non-null: `Only these tools: <names>.`
   - `exclude_tools` present: `All tools in the access class except: <names>.`
3. Per-grant mode sentence (when the grant's `mode` field is present):
   - `"enforce"`: `This grant always enforces: its denials block even when the policy mode is observe.`
   - `"observe"`: `This grant is always observe-only: its denials are recorded, never blocked.`
4. Purpose sentence (when `description` is present):
   `Purpose: <description>.`

The unmatched line depends on the block-3 resolved mode:

- enforce: `Any domain not matched above is denied.`
- observe: `Any domain not matched above would be denied; in this mode that denial is recorded, not blocked.`

When `grants` is empty, block 4 is exactly two lines (no legend, no list):

- enforce:
  ```
  Where agents may read and write: nowhere. This policy grants no domains.
  Every tool call on every page is denied.
  ```
- observe:
  ```
  Where agents may read and write: nowhere. This policy grants no domains.
  Every tool call would be denied; in this mode those denials are recorded, not blocked.
  ```

Block 5, settings. Partition the manifest `config` array by `level`, preserving array
order within each partition. `<value>` is `serde_json::to_string` of the entry value
(compact: booleans bare, strings quoted, arrays compact). `<desc>` is the key's
`description` from the registry, verbatim.

- When at least one `"mandatory"` entry exists, emit:
  ```
  Settings locked by the organization (users cannot change these):
    - <key> = <value> (<desc>)
  ```
  (one `  - ` line per mandatory entry, in array order)
- When at least one `"recommended"` entry exists, emit:
  ```
  Org-recommended defaults (users may change these):
    - <key> = <value> (<desc>)
  ```
- When ANY config entry exists, the block continues with the line:
  `All other settings keep their user, preset, or built-in values.`
- When at least one `"mandatory"` entry exists, the block ends with the line (shared
  format 1.3, the downgrade rule):
  `If a user loads this file themselves instead of the organization installing it as the org policy file, the locked entries above become user-level defaults and nothing is locked.`
- When the `config` array is absent or empty, the whole block is exactly one line:
  `This policy locks no settings and sets no defaults; users keep control of every setting.`

Block 6, denial (one line), by the block-3 resolved mode:

- enforce: `On a denial the agent receives a plain-text message with a stable denial id, in the form 'Denied (D-xxxxxxxx): ...'. Hand that id to the policy administrator: it identifies the exact rule and policy version that produced the denial.`
- observe: `On a would-deny the agent sees the ordinary tool result and no denial text. The denial id appears only in the audit record, as decision 'shadow_deny'.`

Block 7, warnings. Emitted only when at least one warning exists:

```
Warnings:
  - <warning>
```

Warning collection order is deterministic: iterate grants in manifest order; for each
grant emit first the bare-write lint (when `access` is `"write"`; shared format 4.3),
then one non-ASCII-pattern lint per offending pattern in `domains` order (shared format
5.1). Exact lines:

- `grant '<id>': access "write" does not include read; agents can act on pages they cannot read. Most policies want "read" or "all".`
- `grant '<id>': domain pattern '<pattern>' contains non-ASCII characters; author IDN domains in punycode (A-label) form.`

### 4. Fixed templates (user configuration file)

For an input file without a top-level `schema` member (shared format 1.1: optional
`preset`, optional `config` flat map), render these blocks, same joining rules:

1. `User configuration file (not a policy manifest).`
2. Preset line:
   - valid preset present: `Preset: <preset>.`
   - absent: `Preset: none (the built-in defaults apply).`
   - unknown preset string: render the absent form AND add the warning
     `unknown preset '<preset>'; the built-in defaults apply.`
3. Settings:
   - `config` map non-empty (valid entries only, file order, `preserve_order` keeps it):
     ```
     User settings:
       - <key> = <value> (<desc>)
     ```
   - otherwise the single line: `User settings: none.`
4. `Nothing here is locked: only an org policy file can lock settings.`
5. Warnings block (same `Warnings:` format as manifests) when any exist, in file order:
   - `unknown key '<key>' is ignored.`
   - `invalid value for '<key>' is ignored (<one-line reason naming the expected type or constraint>).`

### 5. CLI wiring in `src/main.rs`

- Add a `Policy(PolicyArgs)` variant to the `Command` enum with doc comment
  `/// Inspect and preview policy files.`. `PolicyArgs` holds
  `#[command(subcommand)] command: PolicyCommand`; `PolicyCommand` has one variant for
  now, `Explain(ExplainArgs)`, doc comment
  `/// Render a policy manifest or config file as plain sentences.`, with a single
  required positional argument `file: std::path::PathBuf` (value name `FILE`). Nesting
  under a `policy` subcommand is deliberate: the simulate task adds its sibling variant
  later.
- The `match` arm runs synchronously (no tokio runtime, like `Doctor`): call
  `browser_mcp::policy::explain::explain_file(&args.file)`; on `Ok(text)` write it to
  stdout with `print!` (the text already ends with `\n`); on `Err`, propagate via `?`
  into `main`'s `anyhow::Result` so the error prints to stderr and the process exits
  nonzero. Nothing else goes to stdout: stdout is exactly the rendering, so shells can
  redirect it.
- Manifest validation failures (unknown schema version, duplicate grant ids, unknown
  tool names, unknown config keys, invalid domain patterns, invalid mode strings) are
  ERRORS from the prerequisite parser: explain reports them and exits nonzero. It never
  renders a best-effort explanation of an invalid manifest -- a half-explained policy is
  a misstated policy.

### 6. Example manifests and goldens

Create the `examples/` directory at the repository root with exactly these three files
(verbatim; they are schema-2 replacements for the superseded SPEC Appendix A examples).
Deliberately NOT authored: a `developer-unrestricted` example. Under ADR-0013,
unrestricted IS the absence of a manifest; a manifest with empty grants means deny-all,
the opposite. Do not invent one.

`examples/enterprise-healthcare.json`:

```json
{
  "schema": 2,
  "name": "enterprise-healthcare",
  "version": "2026.07.1",
  "mode": "enforce",
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
      "exclude_tools": ["javascript_tool"],
      "description": "ServiceNow incident and change management without arbitrary JS"
    },
    {
      "id": "epic-read",
      "domains": ["epic.geisinger.org"],
      "access": "read",
      "description": "EHR read-only for clinical data review"
    },
    {
      "id": "research",
      "domains": ["*.gartner.com", "*.ieee.org", "scholar.google.com"],
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

`examples/qa-staging.json` (exercises observe mode, a per-grant enforce override, a
positive `tools` list, and the bare-write lint):

```json
{
  "schema": 2,
  "name": "qa-staging",
  "version": "2026.07.1",
  "mode": "observe",
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
      "description": "Full automation on staging"
    },
    {
      "id": "production-readonly",
      "domains": ["geisinger.org", "*.geisinger.org"],
      "access": "read",
      "mode": "enforce",
      "description": "Read-only verification on production"
    },
    {
      "id": "form-writer",
      "domains": ["forms.staging.geisinger.org"],
      "access": "write",
      "tools": ["form_input"],
      "description": "Submit test forms only"
    }
  ]
}
```

`examples/research-read-only.json` (exercises no identity, no mode field, no config, no
description):

```json
{
  "schema": 2,
  "name": "research-read-only",
  "version": "1",
  "grants": [
    {
      "id": "research",
      "domains": ["*.arxiv.org", "scholar.google.com"],
      "access": "read"
    }
  ]
}
```

Goldens live at `tests/fixtures/explain/enterprise-healthcare.txt`,
`tests/fixtures/explain/qa-staging.txt`, `tests/fixtures/explain/research-read-only.txt`.
Each golden is the exact `explain_file` output for its example, committed with LF line
endings. Generation procedure: implement the renderer, run
`cargo run -- policy explain examples/<name>.json > tests/fixtures/explain/<name>.txt`
once per example, then REVIEW EVERY LINE of each golden against the templates in
sections 3 above before committing. A golden that pins a wrong sentence is exactly the
defect this task exists to prevent; do not commit unreviewed output.

For orientation (NOT a byte-exact fixture: the hash and the two audit-key descriptions
depend on the committed file bytes and the registry prerequisite's description strings),
`enterprise-healthcare.txt` will read:

```
Policy 'enterprise-healthcare', version 2026.07.1.
Content hash: <64 lowercase hex of the committed file>.

Prepared for 'GEISINGER\jdoe', resolved by managed_config. Groups: Dept-EA, App-ServiceNow-Admin, App-Epic-ClinicalRead.

Mode: enforce. Calls the grants below do not permit are blocked.

Where agents may read and write, in match order (the first matching domain wins):
(a pattern like 'example.com' matches only that exact host; '*.example.com' matches its subdomains and never example.com itself)
  1. Full access on servicenow.geisinger.org: agents may read and act. All tools in the access class except: javascript_tool. Purpose: ServiceNow incident and change management without arbitrary JS.
  2. Read-only on epic.geisinger.org: agents may read pages but not act on them. Purpose: EHR read-only for clinical data review.
  3. Read-only on *.gartner.com, *.ieee.org, scholar.google.com: agents may read pages but not act on them. Purpose: External research resources.
  4. Read-only on confluence.geisinger.org, sharepoint.geisinger.org: agents may read pages but not act on them. Purpose: Internal documentation.
Any domain not matched above is denied.

Settings locked by the organization (users cannot change these):
  - audit.enabled = true (<registry description of audit.enabled>)
  - audit.destination = "file" (<registry description of audit.destination>)
Org-recommended defaults (users may change these):
  - content.security.secrets.redact = true (Redact values of secret fields (password/OTP/payment) in read_page output.)
All other settings keep their user, preset, or built-in values.
If a user loads this file themselves instead of the organization installing it as the org policy file, the locked entries above become user-level defaults and nothing is locked.

On a denial the agent receives a plain-text message with a stable denial id, in the form 'Denied (D-xxxxxxxx): ...'. Hand that id to the policy administrator: it identifies the exact rule and policy version that produced the denial.
```

### 7. Tests

Unit tests, inline `#[cfg(test)]` in `src/policy/explain.rs`, pinning exact sentence
strings (assert with `==` on full lines, not `contains`, wherever the line is fully
determined):

1. Access sentences: a synthetic grant rendered under each of `read`, `write`, `all`
   produces the exact block-4 sentence.
2. Bare-write lint: the `write` grant from test 1 produces exactly the write warning
   line; `read` and `all` grants produce none.
3. Non-ASCII pattern lint: a grant with a non-ASCII domain pattern produces exactly the
   punycode warning line.
4. Per-grant mode sentences: `mode: "enforce"` and `mode: "observe"` on a grant produce
   their exact sentences; a grant without `mode` produces neither.
5. Mode line and suffixes: manifest `mode` present yields no suffix; a mandatory
   `governance.mode` config entry yields the locked suffix; a recommended entry yields
   the user-may-change suffix; none of the above yields the built-in-default suffix with
   `Mode: enforce.`. The observe base sentence ends exactly with
   `Observation is not protection.`
6. Empty grants: both two-line renderings (enforce and observe) are exact.
7. No identity: the exact `No identity block:` line.
8. Settings block: mandatory-only, recommended-only, both, and empty inputs produce the
   exact section headers, entry lines, closing line, and (mandatory only) the downgrade
   line; the empty input produces exactly the single locks-nothing line.
9. Denial block: exact line under enforce and under observe.
10. Determinism: parse the same manifest twice, render both, assert the two strings are
    byte-identical; assert the output ends with exactly one `\n` and contains no `\r`.
11. User config file: a file with a preset and two valid entries renders the exact
    section-4 blocks; an unknown key and an invalid value produce their exact warning
    lines; an unknown preset produces its warning and the `Preset: none` line.

Integration test, new file `tests/policy_explain.rs`, spawning the built binary with
`env!("CARGO_BIN_EXE_browser-mcp")` (pattern: `tests/mcp_protocol.rs` line 22):

1. Golden equality, one test per example: run
   `browser-mcp policy explain examples/<name>.json`, assert exit success, assert stdout
   equals the committed golden. Compare after stripping every `\r` byte from BOTH sides
   so a git `autocrlf` checkout cannot break the suite; the renderer itself must emit no
   `\r` (pinned by unit test 10).
2. Invalid manifest: write a temp file with `"schema": 99` (use `std::env::temp_dir()`
   plus a unique name; remove it after), run explain on it, assert nonzero exit, empty
   stdout, non-empty stderr.
3. Missing file: explain on a path that does not exist exits nonzero with empty stdout.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. G16 does not touch the tool
   surface at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. G16 changes no extension file; the renderer and CLI live entirely in
   the binary.
3. All-open stays first-class: G16 adds a read-only CLI command and touches neither
   `src/dispatch.rs`, `src/mcp/server.rs`, nor any tool path, so mcp-server behavior
   with no manifest and default config stays byte-identical to today. Keep it that way:
   if you find yourself editing dispatch or the server loop, stop.
4. ASCII only in ALL code, docs, templates, and rendered output: no em-dashes, arrows,
   or curly quotes anywhere, including comments, this prompt's templates, the example
   manifests, and the goldens. Input VALUES echoed into output (principal, description)
   are the one pass-through exception.
5. The engine is truthful: the observe rendering must state plainly that nothing is
   blocked and that observation is not protection (the block-3 and block-6 templates do;
   render them verbatim). Never render an invalid manifest; error out instead.
6. No new runtime dependencies and no new dev-dependencies. `serde_json` and `clap`
   already cover everything; the content hash comes from the manifest prerequisite. If
   `sha2` is not yet in `Cargo.toml`, the prerequisite has not landed; stop.
7. Rust 2021 edition; `thiserror` for `ExplainError` (library code; `anyhow` only in
   `main.rs`); doc comments on every public item and a module doc comment; `cargo fmt`
   clean; `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline;
   integration tests in `tests/`.
8. Do NOT copy code from other projects; implement from the behavior described here.

Task-specific:

9. The renderer is pure and machine independent: no clock, no randomness, no HashMap
   iteration order anywhere in the output path (the manifest and config types preserve
   order via `preserve_order`; the registry is an ordered const slice), no platform
   paths, no environment reads. Byte-identical output on Windows, macOS, and Linux is a
   requirement, not a nicety.
10. Use the template strings of Required behavior sections 3 and 4 verbatim. If a
    template seems wrong or a case seems unrepresentable with the prerequisite's types,
    stop and report; do not improvise a sentence. A misstated sentence is a policy
    misstatement.
11. One renderer: the CLI, the goldens, and the future import preview all go through
    `explain_manifest`. Do not fork a second formatting path (no separate "short" mode,
    no alternate wording anywhere).
12. Reuse prerequisite types and functions: the parsed manifest type, its validator, the
    content hash, the `Mode` enum, the registry `KeyDef` descriptions, and the
    user-config loader if present. Duplicating any of them is a defect.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including the new unit tests in
   `src/policy/explain.rs`, the new `tests/policy_explain.rs`, and every pre-existing
   test unchanged (notably `tests/tool_schema_fidelity.rs`).
4. Manual: `cargo run -- policy explain examples/enterprise-healthcare.json` prints the
   golden text and nothing else to stdout; the same command redirected to a file
   produces bytes identical to `tests/fixtures/explain/enterprise-healthcare.txt`
   (modulo any `\r` a Windows checkout added to the golden). Repeat for the other two
   examples.
5. Manual review gate: read each committed golden line by line against the Required
   behavior templates and against its example manifest. Confirm in particular that
   `qa-staging.txt` says `Mode: observe (shadow).`, contains
   `Observation is not protection.`, renders the `production-readonly` grant with
   `This grant always enforces:`, renders `form-writer` with `Only these tools:
   form_input.`, and carries the `form-writer` write warning under `Warnings:`.
6. `browser-mcp policy explain` on a missing file and on a `"schema": 99` file exits
   nonzero with the error on stderr and nothing on stdout.
7. Non-ASCII scan of everything this task adds, for example
   `rg -n "[^\x00-\x7F]" src/policy/explain.rs examples tests/fixtures/explain tests/policy_explain.rs docs/tasks/stage-2/g16-policy-explain.md`;
   there must be no hits.
8. Build note: if `target/debug/browser-mcp.exe` is locked by a running MCP session,
   rename it aside (`mv target/debug/browser-mcp.exe
   target/debug/browser-mcp.exe.old-1`) and rebuild. No extension reload is needed; no
   MCP client restart is needed to test the CLI (it is a plain subcommand).

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Localization or i18n of any kind. The template set is fixed English; there is no
  language switch, no message catalog, no `--lang` flag.
- HTML output, Markdown output, JSON output, colored/ANSI output, or any `--format`
  flag. Plain LF-terminated text on stdout is the only format.
- The import UI / import-preview surface itself. G16 only EXPORTS the renderer; the
  surface that calls it for shared-manifest import is a future task.
- `policy simulate`, `policy init`, template galleries, and JSON Schema generation
  (ADR-0020 commitments 1 and 3; separate stage-2 tasks). Do not pre-create their CLI
  variants; `PolicyCommand` gets exactly one variant here.
- Explaining the LIVE merged machine state (org file + user file + preset). That is
  `config list` territory. `explain_file` renders one file in isolation and must not
  read platform config paths.
- Manifest parsing, validation, content hashing, the `Mode` enum, registry growth, or
  the user-config loader. Prerequisite tasks own them; G16 only calls them.
- Any change to `src/dispatch.rs`, `src/mcp/server.rs`, enforcement, audit, the
  extension, the installer, the IPC transport, or `src/mcp/schemas/tools.json`.
- A `developer-unrestricted.json` example manifest (unrestricted = no manifest under
  ADR-0013; see Required behavior section 6).
- Amending `docs/SPEC.md` Appendix A. The supersession is tracked in the shared format
  doc's "SPEC updates needed" list; the SPEC scrub is a separate docs task.
- `.gitattributes`, CI workflow, or packaging changes. The `\r`-stripping compare in the
  golden test is the whole line-ending story for this task.
