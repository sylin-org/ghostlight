# G03: Config CLI (list / get / set with source and lock display)

## Goal

Add a `config` subcommand to the `browser-mcp` binary with three actions:

- `browser-mcp config list` prints an ASCII table of every registered configuration
  key: key name, effective value, source layer, locked marker, and the one-line
  description from the registry.
- `browser-mcp config get <key>` prints one key's effective value plus its source
  layer and lock state.
- `browser-mcp config set <key> <value>` validates the value via the typed key
  registry, writes it to the user-layer config file preserving all content the CLI
  does not own, and REFUSES locked keys with a message naming the org layer as the
  source (the managed-by-your-organization moment).

Exit codes: 0 on success; 1 on unknown key, invalid value, lock refusal, or an
unwritable/unparseable user config file. Output is plain ASCII and stable enough to
grep. The CLI is a thin presentation surface: layer resolution and value validation
live in the policy module (built by prerequisite tasks); the CLI renders and writes.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md`, the reconciled format reference. Read it
  before writing any code; its names are authoritative. Load-bearing sections for
  this task: 1.1 (user config file location and format), 2 (layer model), 2.1
  (resolved-value triple: `value`, `source`, `locked`), 3 (key registry, value
  types, initial key set), and 9.2 (the exact "managed by your organization"
  wording, which the CLI refusal must echo).
- The stage-2 configuration-registry and layered-resolution task(s): whichever
  G-tasks grow `KeyDef` to typed values with constraints (shared format section
  3.3), register the seven stage-2 keys (section 3.4), load the user config file
  and the org policy file's config entries, and expose a resolver that returns the
  resolved triple per key (section 2.1) plus a validation function for
  (key, candidate value). G03 consumes that machinery; it does not implement layer
  precedence or constraint checking. If the typed registry or the resolver is not
  yet present in `src/policy/`, STOP and land the prerequisite first; do not invent
  a resolver inside this task.
- All release-1 (stage-1) tasks in `docs/tasks/release-1/` are assumed landed.

Because the prerequisites reshape `src/policy/mod.rs` (and possibly add sibling
modules under `src/policy/`), the "Current behavior" section below records the tree
as it stands today. Do NOT trust it as the state you will edit. Re-read every file
named below before changing it and integrate against the types and function names
the prerequisites actually produced.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP
server (JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome
native-messaging host; a thin Manifest V3 extension executes CDP commands.
Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles run as separate OS processes bridged by tokio-native
named-pipe (Windows) or Unix-domain-socket IPC. The binary also has synchronous
installer subcommands (`install`, `uninstall`, `doctor`) that run without an async
runtime; the `config` subcommand added here is the same kind of synchronous CLI
role.

Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance
layer per ADR-0013 (separable overlay; all-open stays first-class), ADR-0018
(observe-then-enforce sequencing), ADR-0019 (layered configuration: typed key
registry, presets, org locks, CLI and extension surfaces, no embedded web server),
and ADR-0020 (org policy experience). This task delivers the CLI surface promised
by ADR-0019: the user can see every setting, see WHERE each value comes from, and
edit the user layer; a key locked by the organization is visibly locked and refuses
edits with plain language.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The
shared format doc `docs/tasks/stage-2/00-shared-format.md` is the reconciled single
source for file formats, field names, and locations. Use ITS names verbatim: the
source enum is exactly `org_mandatory`, `user`, `org_recommended`, `preset`,
`builtin` (section 2.1); the user config file is section 1.1's `config.json` with
optional `preset` and `config` members.

Key files for this task:

- `src/main.rs`, the clap CLI shell. Subcommands are declared here as clap types
  and converted into library types via `From` impls (see `InstallArgs` /
  `InstallOptions`). Your `config` subcommand follows the same pattern.
- `src/policy/mod.rs`, the governance module and typed key registry. Your new CLI
  module is declared here.
- `src/policy/cli.rs`, NEW in this task: the config CLI implementation.
- `src/error.rs`, the library error enum (`thiserror`). You add one variant.
- `src/install/mod.rs`, the installer: the CLI conventions to match (synchronous
  entry points returning `crate::Result<()>`, plain `println!` output, errors
  surfaced through `main`'s `anyhow::Result`).
- `src/install/native_host.rs`, provides `pub fn write_file_atomic(path, contents)`
  (line 255): creates parent directories, writes a `.tmp` sibling, renames over the
  target. Reuse it for the user config file write; do not write a second atomic
  writer.

## Current behavior

Verified against the tree as of this writing (prerequisites will have changed some
of this; re-read before editing):

- `src/main.rs` declares `enum Command` (lines 43-53) with exactly four variants:
  `Install`, `Uninstall`, `Doctor`, `Status`. `main()` (line 160) returns
  `anyhow::Result<()>` and matches `Cli::parse()` at lines 173-195; each installer
  arm calls a `browser_mcp::install::run_*` function that returns
  `browser_mcp::Result<()>`, so a returned `Err` prints `Error: <message>` to
  stderr (anyhow's termination behavior) and exits with code 1. Native-host role
  detection (line 169) happens BEFORE clap parsing and is keyed on a
  `chrome-extension://` argument; a `config` subcommand cannot collide with it.
- There is no `config` subcommand and no code that reads a user config file. The
  mcp-server role builds its configuration as `Config::default()` at
  `src/mcp/server.rs` line 28.
- `src/policy/mod.rs` today holds the registry seed:
  `KeyDef { key, description, minimal_default: bool }`, the `KEYS` table with one
  entry (`content.security.secrets.redact`), and `Config` with `Config::minimal()`.
  The registry-growth prerequisite replaces `minimal_default: bool` with typed
  values and constraints and registers the seven keys of shared format section 3.4.
- `src/error.rs` declares `pub enum Error` (line 10) with variants `Protocol`,
  `NativeMessaging`, `Ipc`, `SessionBusy`, `Json`, `Io`, `MissingExtensionId`,
  `InvalidExtensionId`, `HostRegistration`, `ClientRegistration`, `MergeConflict`,
  `Unsupported` (through line 57). No config-related variant exists.
- `Cargo.toml` already has `serde_json` with the `preserve_order` feature
  (line 12), `dirs = "6"` (line 18), `clap`, and `thiserror`. No new dependency is
  needed for this task.
- The user config file per shared format section 1.1 resolves to
  `dirs::config_dir().join("browser-mcp").join("config.json")` on every platform
  (Windows `%APPDATA%\browser-mcp\config.json`, macOS
  `~/Library/Application Support/browser-mcp/config.json`, Linux
  `~/.config/browser-mcp/config.json`). Absence of the file is normal and means
  "no user layer". The prerequisite resolver may already expose a path helper for
  it; if so, call that helper instead of re-deriving the path.

## Required behavior

### 1. Clap surface in `src/main.rs`

Add a fifth variant to `enum Command`:

```rust
/// Inspect and edit the layered configuration (list / get / set).
Config(ConfigArgs),
```

Add the clap types, mirroring the existing `InstallArgs` style:

```rust
#[derive(Debug, Args)]
struct ConfigArgs {
    #[command(subcommand)]
    action: ConfigAction,
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Show every key: effective value, source layer, lock state, description.
    List,
    /// Show one key's effective value, source layer, and lock state.
    Get { key: String },
    /// Set a key in the user layer. Refused when the organization locks the key.
    Set { key: String, value: String },
}
```

Add a `From<ConfigArgs> for browser_mcp::policy::cli::ConfigCommand` impl (same
pattern as `From<InstallArgs> for InstallOptions`) and a match arm in `main()`:

```rust
Cli {
    command: Some(Command::Config(args)),
    ..
} => browser_mcp::policy::cli::run(args.into())?,
```

The library crate must stay clap-free: `ConfigArgs` / `ConfigAction` live in
`src/main.rs` only; `ConfigCommand` (below) is plain Rust.

### 2. New module `src/policy/cli.rs`

Create `src/policy/cli.rs` and declare it in `src/policy/mod.rs` with
`pub mod cli;` next to the existing module declarations. Module-level doc comment:
this is the ADR-0019 CLI surface over the layered configuration registry; it
renders the resolved triple (value, source, locked) and writes the user layer only;
resolution and validation live in the registry, never here.

Public shape:

```rust
/// A parsed `browser-mcp config` invocation.
pub enum ConfigCommand {
    List,
    Get { key: String },
    Set { key: String, value: String },
}

/// Run one config CLI command. Success output goes to stdout; failures return
/// `Error::Config`, which the binary surfaces on stderr with exit code 1.
pub fn run(cmd: ConfigCommand) -> crate::Result<()>
```

`run` is synchronous (no tokio runtime), like the installer entry points.

Structure the module so rendering is pure and testable: helpers that BUILD the
output take resolved data in and return `String`; `run` alone performs I/O
(resolver calls, `println!`, file writes). The exact resolver types come from the
prerequisite; adapt to what `src/policy/` actually exposes. What `run` needs from
it, per key: the key name, the registry description, the effective JSON value, the
source (one of `org_mandatory`, `user`, `org_recommended`, `preset`, `builtin`),
and the locked flag (`true` if and only if the source is `org_mandatory`). Do not
re-implement layer precedence in the CLI; call the resolver.

If the resolver emits warnings while loading (unknown keys or type-invalid values
in the user config file, per shared format section 1.1), they go to stderr as
`warning: <text>` lines so stdout stays a clean, greppable table. Warnings do not
change the exit code.

### 3. `config list`

One header line, then one row per registered key, in registry (`KEYS`) declaration
order. Exact format strings:

```rust
println!("{:<40}{:<24}{:<17}{:<8}{}", "KEY", "VALUE", "SOURCE", "LOCKED", "DESCRIPTION");
println!("{:<40}{:<24}{:<17}{:<8}{}", key, value, source, locked, description);
```

where per row:

- `key`: the dotted key name.
- `value`: the effective value as compact JSON via `serde_json::to_string`
  (booleans `true`/`false`, integers bare, strings and enums quoted like `"file"`,
  lists like `["example.com","*.example.com"]`).
- `source`: the source enum string verbatim (`org_mandatory`, `user`,
  `org_recommended`, `preset`, `builtin`).
- `locked`: the literal `locked` when locked, the literal `-` otherwise.
- `description`: the registry description, last so its width is unconstrained.

Values wider than a column shift that row right; that is accepted (the widths are
minimums). No color, no ANSI escapes, no box-drawing characters. Exit 0.

Example (illustrative values):

```
KEY                                     VALUE                   SOURCE           LOCKED  DESCRIPTION
engine.connection.first_call_wait_ms    5000                    builtin          -       Upper bound on the first-call wait for the extension handshake.
content.security.secrets.redact         true                    org_mandatory    locked  Redact values of secret fields (password/OTP/payment) in read_page output.
audit.destination                       "file"                  preset           -       Where audit records are written.
```

### 4. `config get <key>`

For a registered key, print exactly five lines to stdout and exit 0:

```
key: <key>
value: <compact JSON value>
source: <source enum string>
locked: <yes|no>
description: <registry description>
```

For an unknown key, return
`Error::Config(format!("unknown config key '{key}' (run 'browser-mcp config list' to see all keys)"))`
so the process prints `Error: unknown config key ...` on stderr and exits 1.

### 5. `config set <key> <value>`

Steps, in order; the first failure stops the command with exit 1 and writes
nothing:

1. Unknown key: same `Error::Config` message as `get`.
2. Lock check: resolve the key's triple. If `locked` is true (source is
   `org_mandatory`), refuse with exactly this message (no file access, no write):

   ```
   Error::Config(format!(
       "{key} is managed by your organization (source: org_mandatory); 'config set' cannot override it"
   ))
   ```

   The refusal applies even when the requested value equals the org value. This
   wording must stay in agreement with the shared format section 9.2 `locked`
   error ("This setting is managed by your organization."); if a shared constant
   for that wording exists in the policy module by the time this task runs, build
   the message from it rather than duplicating the string.
3. Parse the raw CLI string into a `serde_json::Value` according to the key's
   registered type (shared format section 3.2):
   - bool: the raw string must be exactly `true` or `false`; anything else is
     invalid with detail `expected 'true' or 'false'`.
   - uint: ASCII decimal digits only, parsed as `u64`; a parse failure is invalid
     with detail `expected an unsigned integer`. Range checking is the registry
     validator's job in step 4.
   - enum: the raw string is taken verbatim (no quotes); variant membership is the
     validator's job.
   - string: the raw string is taken verbatim (an empty string is legal input;
     for `audit.file.path` it means "platform default").
   - string list: the raw string must parse as a JSON array of strings, e.g.
     `["example.com","*.example.com"]`; otherwise invalid with detail
     `expected a JSON array of strings, e.g. ["example.com","*.example.com"]`.
     Duplicate and per-element rules are the validator's job.
4. Validate the parsed value with the registry's validation function (the
   prerequisite provides it; do NOT duplicate range, variant, duplicate, or
   domain-pattern logic in the CLI). Any parse or validation failure returns
   `Error::Config(format!("invalid value for {key}: {detail}"))`, where `detail`
   is the step-3 detail for parse failures or the validator's constraint text for
   validation failures (e.g. `expected an integer between 0 and 60000`,
   `expected one of: file, stderr`).
5. Write the user layer. Read the user config file (section 1.1 path); a missing
   file starts from `{}`. Parse with `serde_json` (order preserved). Refuse
   without writing, via
   `Error::Config(format!("cannot update {path}: {reason}"))`, when the file
   exists but is not valid JSON (`not valid JSON: <parse error>`), when the root
   is not a JSON object (`root is not a JSON object`), or when a `config` member
   exists but is not a JSON object (`'config' is not a JSON object`). Otherwise
   create the `config` object if absent, set `config[<key>] = <parsed value>`
   (replacing any previous value for that key), and leave EVERYTHING else in the
   document untouched at the value level: the `preset` member, unknown top-level
   members, and other `config` entries, including entries for keys this build does
   not recognize. Serialize with `serde_json::to_string_pretty` plus a trailing
   newline and write with `crate::install::native_host::write_file_atomic`. The
   write always happens on success, even when the new value equals the old one
   (idempotent by value).
6. Success output, exactly two lines to stdout, exit 0:

   ```
   <key> = <compact JSON value>
   written to the user layer: <absolute path to config.json>
   ```

Note the effective source for the key becomes `user` after a successful set (the
user layer outranks `org_recommended`, `preset`, and `builtin`; `org_mandatory`
was refused in step 2). Setting a key over an `org_recommended` value is allowed
by design. A running mcp-server session does not reload; new values apply per each
key's own semantics on the next start (live application is the settings-protocol
task's concern, not this one).

### 6. Error variant in `src/error.rs`

Add one variant:

```rust
/// A `config` CLI request failed: unknown key, invalid value, org-locked key, or
/// an unusable user config file. Display is the full user-facing message.
#[error("{0}")]
Config(String),
```

All messages above are specified WITHOUT an `error:` prefix; the binary's existing
`anyhow` termination path prints them as `Error: <message>` on stderr and exits 1,
matching the installer subcommands. Clap's own usage errors (unknown subcommand,
missing argument) keep clap's default exit code 2; the 0/1 contract applies to
well-formed invocations.

### 7. Tests

Inline unit tests in `src/policy/cli.rs` (`#[cfg(test)] mod tests`). Pure helpers
take injected data; the user-file writer takes the target path as a parameter so
tests use `std::env::temp_dir()` subdirectories (mirror the tempdir hygiene of the
tests in `src/install/mod.rs`). Required tests, by behavior:

1. List rendering: given a fabricated resolved set covering all five source values
   and at least one locked key, the rendered output matches the pinned header and
   row strings exactly (assert on full lines, including the `locked` and `-`
   markers and compact-JSON values for a bool, a uint, an enum/string, and a
   string list).
2. Get rendering: the five-line output is pinned exactly for one unlocked and one
   locked key (`locked: no` / `locked: yes`).
3. Value parsing per type: accepted and rejected inputs for bool (`true`, `false`,
   reject `True`, `1`, `yes`), uint (`0`, `60000`, reject `-1`, `1.5`, `1e3`,
   `abc`), string list (accept `["a.com","*.a.com"]`, reject `a.com`, `[1]`,
   `{"a":1}`), and string passthrough (including the empty string).
4. Lock refusal: a locked key produces `Error::Config` whose Display equals the
   exact step-2 message, and the user config file is not created or modified
   (assert on a temp path).
5. User-file write preservation: starting from
   `{"preset":"safe","config":{"content.security.secrets.redact":false},"future_member":{"x":1}}`,
   setting a different key preserves `preset`, `future_member`, and the existing
   config entry byte-for-value; setting the SAME key replaces only its value. A
   missing file is created (with parent directories). A file containing invalid
   JSON or a non-object root is refused and left byte-for-byte untouched.
6. Unknown key: `get` and `set` both produce the exact unknown-key message.

No new integration test file: the real config path depends on machine state
(`dirs::config_dir()` cannot be injected via environment variables on Windows), so
subprocess tests of `config` would be environment-dependent. The path-injected
unit tests above carry the coverage.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or
   descriptions. `tests/tool_schema_fidelity.rs` must pass unchanged. This task
   does not touch the tool surface at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions
   in extension JS. This task changes no extension file. The extension settings
   surface (shared format section 9) is a separate task.
3. All-open stays first-class: with no manifest and default config, runtime
   behavior is byte-identical to today. This task guarantees that by not touching
   `src/dispatch.rs`, `src/mcp/server.rs`, or any tool code; the `config`
   subcommand only reads resolved state and writes the user config file when the
   user explicitly asks.
4. ASCII only in ALL code, comments, output strings, and docs: no em-dashes,
   arrows, or curly quotes. CLI output is plain ASCII with no ANSI escapes.
5. The engine is truthful: the lock refusal names the org layer plainly
   (`managed by your organization`, `source: org_mandatory`); never soften it to a
   generic "permission denied", and never claim a write happened when it was
   refused.
6. No new runtime dependencies. `clap`, `serde_json` (with `preserve_order`),
   `dirs`, and `thiserror` already exist in `Cargo.toml` and suffice. No table
   crates (`comfy-table`, `tabwriter`, `prettytable`), no `colored`, no
   `once_cell`.
7. Rust 2021 edition; `thiserror` for the new error variant; doc comments on every
   public item (module, `ConfigCommand` and its variants, `run`); `cargo fmt`
   clean; `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline in
   `cli.rs`.
8. Do NOT copy code from other projects; implement from the behavior described
   here.
9. Use the shared format doc's names verbatim: source strings `org_mandatory`,
   `user`, `org_recommended`, `preset`, `builtin`; file locations from section 1.1;
   the user config file's `preset` and `config` member names. Do not invent
   alternate spellings (`orgMandatory`, `default`, `system`).
10. The CLI never resolves layers or checks constraints itself; it calls the
    registry/resolver from the prerequisite task. If the needed function does not
    exist, stop and report; do not fork the logic.
11. The user config file write is value-level and preserving: only
    `config[<key>]` changes; `preset`, unknown members, and unrecognized config
    entries survive every `set`. Never truncate-then-write; use
    `write_file_atomic`.
12. There is no code path, flag, or environment variable by which this CLI writes
    the org policy file or an org layer. `set` writes the user layer only.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including the new unit tests in
   `src/policy/cli.rs`, `tests/tool_schema_fidelity.rs` unchanged, and every
   pre-existing test.
4. Manual smoke (PowerShell; check `$LASTEXITCODE` after each):
   - `cargo run -- config list` prints the header plus one row per registered key,
     registry order, exit 0.
   - `cargo run -- config get audit.destination` prints the five pinned lines,
     exit 0.
   - `cargo run -- config set audit.destination stderr` prints the two success
     lines, exit 0; `config get audit.destination` now shows `source: user`; the
     file at `%APPDATA%\browser-mcp\config.json` contains the entry and any
     pre-existing content.
   - `cargo run -- config set audit.destination bogus` exits 1 with
     `Error: invalid value for audit.destination: ...`.
   - `cargo run -- config set no.such.key true` exits 1 with the unknown-key
     message.
   - `cargo run -- config get no.such.key` exits 1 with the unknown-key message.
   - Lock refusal is pinned by unit test 4. If the org-policy prerequisite is
     landed, additionally verify end to end: place a policy file with a
     `"level": "mandatory"` config entry at the section 1.2 path
     (`%ProgramData%\browser-mcp\policy.json` on Windows), confirm `config list`
     shows `org_mandatory` and `locked` for that key, confirm `config set` on it
     exits 1 with the managed-by-your-organization message, then remove the file.
5. `git status` / `git diff --stat` shows changes ONLY to: `src/main.rs` (clap
   types, `Command::Config` variant, `From` impl, match arm), `src/policy/mod.rs`
   (the `pub mod cli;` declaration), `src/error.rs` (the `Config` variant), and
   the new `src/policy/cli.rs`. `src/mcp/schemas/tools.json` shows no diff.
6. Grep the changed files for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/policy/cli.rs src/main.rs src/error.rs`); there must
   be none.

Build note: if `target/debug/browser-mcp.exe` is locked by a running MCP session,
rename it aside (`mv target/debug/browser-mcp.exe
target/debug/browser-mcp.exe.old-1`) and rebuild. No extension reload is needed
for this task. A running MCP session does not see `config set` changes; restart
the MCP client to pick them up.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Presets (G18). No `config preset` subcommand, no preset names in help text, no
  writing or interpreting the `preset` member of the user config file beyond
  PRESERVING it on `set`. The `preset` SOURCE string still appears in
  list/get output when the resolver reports it; that is display, not preset
  machinery.
- JSON Schema generation (G04). No `config schema` action, no schema output of any
  kind.
- Any TUI or interactive behavior: no prompts, no confirmations, no menus, no
  `Read-Host`-style input, no progress spinners, no color or ANSI styling.
- No `--json` output flag, no machine-readable output mode, no `config export` /
  `import` / `unset` / `reset` actions. Exactly `list`, `get`, `set`.
- No changes to the registry or resolver themselves: no new `KeyDef` entries, no
  constraint logic, no layer-precedence code. Those belong to the prerequisite
  tasks; if they are missing, stop.
- No org-layer surface: no reading knobs to write `policy.json`, no
  `--system`-style flag on `config set`, no lock-bypass flag of any kind.
- No native-messaging settings protocol (shared format section 9); the
  `get_config` / `set_config_key` messages and the extension options page are a
  separate task. Do not touch `src/native/`, `extension/`, or `src/mcp/server.rs`.
- No live-reload plumbing into a running mcp-server session.
- No `doctor` output changes and no `status` changes; governance lines on those
  surfaces belong to other stage-2 tasks.
- No deprecated-key-name handling (shared format section 3.1's rename rule): no
  renames exist yet.
- No changes under `docs/` (including the SPEC; SPEC amendments are tracked in the
  shared format doc's "SPEC updates needed" list and are a separate docs task).
