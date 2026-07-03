# G09: Manifest identity (name, version, content hash, stamped everywhere)

## Goal

Give every policy manifest a computable identity -- its required `name` and `version`
fields plus a SHA-256 content hash over canonical bytes -- so that every logged decision
is attributable to the exact policy version that made it. Stamp that identity into every
audit record (as the `manifest` field, `null` when no manifest is active), and display it
in `browser-mcp doctor` output (and `config list` output, if that command exists when
this task runs). If the manifest engine (G12) has not landed yet, implement identity for
the org policy file as a standalone module so the mechanism and the audit record shape
are exercised end to end, with a clearly marked integration point for the manifest
engine to take over source selection later.

This is ADR-0020 commitment 5: "Manifests carry a name, version, and content hash,
stamped into every audit record and shown by doctor and config list."

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file location, and format in this task comes from it verbatim. Read it
  before writing any code. Load-bearing sections here: 1.2 (org policy file paths),
  1.3 (manifest source selection), 4.1 (required `name` and `version`), 4.2 (the exact
  content-hash definition), 6.1 (the audit `manifest` field), 9.2 (the `get_status`
  `governance.manifest` object), and the crate note at the top (sanctions adding `sha2`).
- G12, the manifest engine (manifest parsing, validation, and source selection per
  shared-format 1.3): CONDITIONAL. If G12 has landed when you run, integrate with it
  (Required behavior section 3, branch A). If it has not landed, build the standalone
  org-policy-file reader (branch B) with the marked integration point.
- G06, the audit subsystem (the JSON Lines record writer of shared-format section 6):
  CONDITIONAL. If `src/audit/` exists when you run, wire the `manifest` field into every
  record (Required behavior section 4, branch A). If it does not exist, deliver the
  serializable identity type plus its pinned JSON shape and a marked integration point
  (branch B); do NOT create `src/audit/` in this task.

Because prerequisite tasks may reshape `src/policy/`, `src/audit/`, and
`src/mcp/server.rs` before G09 runs, the "Current behavior" section below records the
tree as it stands at authoring time. Do NOT trust it blindly as the state you will edit.
Re-read every file named below before changing it, and integrate against the code the
prerequisites actually produced.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles are separate OS processes bridged by tokio-native named-pipe (on
Windows) or Unix-domain-socket (elsewhere) IPC.

Stage 1 (docs/tasks/release-1/) hardened the engine. This is stage 2, the governance
layer: ADR-0013 (separable overlay; all-open stays first-class), ADR-0018
(observe-then-enforce sequencing; audit flight recorder ships before enforcement),
ADR-0019 (layered configuration, typed key registry), ADR-0020 (org policy experience:
explain, simulate, shadow, manifest identity, structured denials). Where `docs/SPEC.md`
and the ADRs disagree, the ADRs win; the shared format doc is the reconciled single
source for formats and names.

Why identity matters here: `policy simulate` replays recorded audit JSONL against a
candidate manifest, denial ids are derived from the manifest hash, and admins trace
decisions in their SIEM. None of that works unless every audit record says exactly which
policy version was in force. ADR-0020's follow-up is explicit: "the audit record shape
must carry manifest identity from its first version so simulate has stable input." G09
is that carrier.

The manifest identity is three facts (shared-format 4.1, 4.2, 6.1):

- `name`: required top-level string in the manifest, e.g. `"acme-clinical-pilot"`.
- `version`: required top-level string, a free-form label, e.g. `"2026.07.1"`.
- `hash`: SHA-256 over the manifest's canonical bytes, rendered as 64 lowercase hex
  characters. COMPUTED by the binary, never stored in the manifest (storing it would
  change the content).

The canonical bytes are defined exactly by shared-format 4.2:

1. Parse the manifest source (file bytes or env variable value) as JSON. A UTF-8 BOM
   (bytes EF BB BF), if present, is stripped before parsing.
2. Re-serialize the parsed value with `serde_json` in compact form (no whitespace),
   preserving object key order as authored (the `preserve_order` feature, already
   enabled in `Cargo.toml`).
3. The canonical bytes are the UTF-8 bytes of that compact serialization.
4. `hash` = SHA-256 over the canonical bytes, 64 lowercase hex characters.

This makes the hash insensitive to whitespace, line endings, and BOM, and sensitive to
content and key order. The same manifest shipped with CRLF or LF hashes identically.

## Current behavior

All facts verified against the working tree at authoring time.

- `src/policy/mod.rs` (104 lines) holds only the typed key registry seed: `pub mod
  redact;` (line 19), `KeyDef` (lines 25-33), the `CONTENT_SECURITY_SECRETS_REDACT` key
  constant (line 39), the one-entry `KEYS` table (lines 43-47), and `Config` with
  `Config::minimal()` (lines 51-70). There is NO manifest type, NO manifest loading, and
  NO identity code anywhere in the crate (grep for `ManifestIdentity`, `sha2`, or
  `canonical` under `src/` finds nothing).
- `src/audit/` does not exist (no such directory). The audit task creates it.
- `src/main.rs`: the CLI (`Cli`, line 26) has a server-role `--manifest
  Option<String>` flag (lines 32-35). `run_server` (line 230) logs it via
  `tracing::info!(?manifest, ...)` but never parses it, and calls
  `browser_mcp::mcp::server::run(browser)` (line 254) without it. Subcommands are
  `Install`, `Uninstall`, `Doctor`, `Status` (lines 43-53); there is NO `config`
  subcommand and NO `policy` subcommand.
- `src/mcp/server.rs`: `run` (line 22) builds `let config = Config::default();` (line
  28) once per session and threads it through `handle_line` to `handle_tools_call`
  (lines 116-155), which calls the no-op seams `dispatch::policy_check(name)` and
  `dispatch::audit(name)` (lines 132-133). No audit record is written anywhere.
- `src/install/mod.rs`: `pub fn run_doctor(_opts: DoctorOptions) -> Result<()>` (line
  719) prints `browser-mcp doctor`, a `Binary:` line (lines 721-724), a `Browsers:`
  section (line 725), and an `MCP clients:` section (line 743), then returns `Ok(())`
  (line 756). `DoctorOptions` (lines 58-61) has a single `verbose: bool` field. There is
  no manifest or governance output in doctor.
- `Cargo.toml` dependencies: `tokio`, `serde` (derive), `serde_json` (with
  `preserve_order`), `clap`, `tracing`, `tracing-subscriber`, `thiserror`, `anyhow`,
  `dirs` (plus Windows-only `winreg`, `windows-sys`). There is NO `sha2`, NO `uuid`, NO
  `chrono`. The shared format doc's crate note sanctions adding `sha2` in stage 2.
- Tests: `tests/tool_schema_fidelity.rs` guards the sacred tool schemas;
  `tests/mcp_protocol.rs` pins the no-manifest stdio behavior. Both must keep passing
  unchanged.

## Required behavior

### 1. Dependency: sha2

If `sha2` is not already in `Cargo.toml` (an earlier stage-2 task may have added it),
add exactly one dependency line to `[dependencies]`:

```toml
sha2 = "0.10"
```

Default features, no feature flags. This is the only new dependency this task may add.
Do NOT add `hex`, `chrono`, `uuid`, or anything else. Render the hash as hex by hand:
iterate the 32 digest bytes and append `format!("{:02x}", byte)` (or the `write!`
equivalent) into a `String`.

### 2. The identity module: `src/policy/identity.rs`

Create `src/policy/identity.rs` and declare it in `src/policy/mod.rs` with
`pub mod identity;` next to the existing module declarations (`pub mod redact;` today;
place it in alphabetical order among whatever `pub mod` lines exist when you run).

Module-level doc comment: this module computes the manifest identity of ADR-0020
commitment 5 (shared-format 4.1 and 4.2); the identity is stamped into every audit
record (shared-format 6.1 `manifest` field) and shown by doctor; the hash is attribution
(which policy version made this decision), NOT authentication (manifest signing is
excluded by SPEC section 10; file ACLs plus the deployment channel are the usage-surface
guard, shared-format 1.2).

The module is `std`-only plus `serde`/`serde_json`/`sha2`/`thiserror`/`tracing`. No
async, no tokio.

#### 2a. The `ManifestIdentity` type

```rust
/// Identity of the active policy manifest (shared-format 4.1, 4.2). Serializes as the
/// audit record's `manifest` object (shared-format 6.1) and the `get_status`
/// `governance.manifest` object (shared-format 9.2): keys `name`, `version`, `hash`,
/// in that order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ManifestIdentity {
    /// Required top-level manifest `name` field.
    pub name: String,
    /// Required top-level manifest `version` field (free-form label).
    pub version: String,
    /// SHA-256 over the canonical bytes, 64 lowercase hex characters.
    pub hash: String,
}
```

Field declaration order is load-bearing: serde emits object keys in declaration order,
and the pinned JSON shape is `{"name":...,"version":...,"hash":...}`.

#### 2b. The error type

A dedicated `thiserror` enum (exact variant names are yours; messages must be as given):

```rust
/// Why a manifest source could not yield an identity.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// The source is not valid JSON.
    #[error("manifest is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// The top-level JSON value is not an object.
    #[error("manifest is not a JSON object")]
    NotAnObject,
    /// A required top-level string field is missing or not a string.
    #[error("manifest is missing required string field '{0}'")]
    MissingField(&'static str),
}
```

#### 2c. The hash and extraction functions

```rust
/// SHA-256 content hash over the canonical bytes of a manifest source
/// (shared-format 4.2). Strips a UTF-8 BOM, parses, re-serializes compactly with
/// authored key order preserved, hashes, and renders 64 lowercase hex chars.
pub fn canonical_hash(source: &[u8]) -> Result<String, IdentityError>

/// Parse a manifest source and extract its identity: required top-level `name` and
/// `version` strings (shared-format 4.1) plus the canonical content hash.
pub fn identity_from_source(source: &[u8]) -> Result<ManifestIdentity, IdentityError>
```

Exact behavior:

- Strip a leading UTF-8 BOM (the byte sequence `EF BB BF`) from `source` if present,
  exactly once, before parsing. Nothing else is trimmed.
- Parse once with `serde_json::from_slice::<serde_json::Value>`. Parse failure is
  `InvalidJson`.
- Canonical bytes: `serde_json::to_vec(&parsed)` (compact is serde_json's default; the
  crate's `preserve_order` feature keeps authored key order). Hash those bytes with
  `sha2::Sha256`; render lowercase hex by hand (section 1).
- `identity_from_source` reuses the SAME parsed value for both the hash and the field
  extraction (parse exactly once). The top-level value must be a JSON object
  (`NotAnObject` otherwise). `name` and `version` must each be present and be JSON
  strings; otherwise `MissingField("name")` / `MissingField("version")`, checked in that
  order.
- Do NOT special-case a `hash` key inside the document. The binary computes the hash; if
  an author put a `hash` field in the file it is ordinary content and participates in
  the hash like any other field. Rejecting unknown fields is manifest validation, which
  belongs to the manifest engine (G12), not here.
- No other validation happens here: no `schema` version check, no `grants` parsing, no
  `config` entries, no `mode`. Identity is deliberately computable even for a manifest
  that would fail full validation, so the flight recorder can attribute records from day
  one; G12 owns rejecting invalid manifests.

### 3. Source resolution

First check whether the manifest engine (G12) has landed: look for manifest parsing /
source-selection code under `src/policy/` (a manifest type with `grants`, a loader
implementing the shared-format 1.3 selection rule). Then follow exactly one branch.

#### Branch A: G12 has landed

- Compute the identity from the EXACT source bytes the engine loaded (the same bytes it
  parsed), at manifest load time, and store the `ManifestIdentity` alongside the parsed
  manifest (a field on the engine's manifest/loaded-policy type, or a value it returns
  next to it). One parse feeding both validation and identity is ideal; if the engine's
  loader does not retain raw bytes, add that retention rather than re-reading the file
  (a re-read could race a file replacement and attribute records to the wrong version).
- Which source is active (org policy file over `--manifest`/env over none) is entirely
  the engine's selection rule (shared-format 1.3); identity attaches to whatever
  manifest the engine made active. Do not duplicate any selection logic.
- Do not implement section 3's Branch B functions; skip to section 4.

#### Branch B: G12 has NOT landed (standalone)

Implement identity for the org policy file only (shared-format 1.2). Add to
`src/policy/identity.rs`:

```rust
/// Platform path of the org policy file (shared-format 1.2), or None when the base
/// directory cannot be resolved. Existence is NOT checked here.
pub fn org_policy_path() -> Option<std::path::PathBuf>

/// Status of the org policy file for identity purposes.
#[derive(Debug)]
pub enum ManifestStatus {
    /// No org policy file exists. Normal; means all-open unless a later task says
    /// otherwise.
    Absent,
    /// The org policy file exists and yielded an identity.
    Active(ManifestIdentity),
    /// The org policy file exists but could not yield an identity.
    Invalid { path: std::path::PathBuf, error: String },
}

/// Read the org policy file (if any) and compute its identity status.
pub fn manifest_status() -> ManifestStatus

/// The identity to stamp into audit records: Some for Active, None otherwise.
pub fn active_manifest_identity() -> Option<ManifestIdentity>
```

- `org_policy_path()` per platform, exactly shared-format 1.2:
  - Windows (`cfg(windows)`): `std::env::var_os("ProgramData")` joined with
    `browser-mcp` then `policy.json` (that is,
    `%ProgramData%\browser-mcp\policy.json`). Return `None` if the env var is absent.
  - macOS (`cfg(target_os = "macos")`): the fixed path
    `/Library/Application Support/browser-mcp/policy.json`.
  - All other unix (`cfg(all(unix, not(target_os = "macos")))`): the fixed path
    `/etc/browser-mcp/policy.json`.
  - Do not use the `dirs` crate here; it has no system-scope (machine-wide) helper for
    these locations.
- `manifest_status()`: if `org_policy_path()` is `None` or the file does not exist,
  return `Absent` (silently; absence is normal). Otherwise read the file bytes with
  `std::fs::read` (bytes, not `read_to_string`; the BOM strip happens in
  `identity_from_source`). A read error or an `IdentityError` yields
  `Invalid { path, error: <Display string of the error> }`.
- `active_manifest_identity()`: maps `Active(id)` to `Some(id)`, everything else to
  `None`. When it returns `None` because of `Invalid`, emit exactly one
  `tracing::warn!` naming the path and the error (the engine is truthful: a present but
  broken policy file must not be silently ignored). `Absent` warns nothing.
- Mark the integration point with this doc comment (verbatim) on `manifest_status`:

```rust
/// INTEGRATION POINT (G12 manifest engine): when the manifest engine lands, the active
/// manifest is selected by shared-format 1.3 (org policy file, else --manifest/env,
/// else none) and identity must be computed from the exact bytes of THAT source at
/// load time. This standalone org-policy-file reader then retires in favor of the
/// engine's loader; identity_from_source and canonical_hash stay as the shared
/// primitives.
```

- In this branch, `--manifest` and `BROWSER_MCP_MANIFEST` sources get NO identity
  (nothing parses them yet); do not read them, and do not claim anything about them in
  any output.

### 4. Stamp the identity into every audit record

Check whether the audit subsystem exists (`src/audit/` directory with a record type and
writer, per shared-format section 6). Follow exactly one branch.

#### Branch A: the audit subsystem exists

- The audit record's `manifest` field (shared-format 6.1) is
  `{ "name": string, "version": string, "hash": string }` when a manifest is active and
  `null` when none is. If the record type already has a manifest field, make sure it is
  populated from the active identity and serializes to exactly that shape (use
  `ManifestIdentity` as the field type if possible; do not maintain two shapes). If the
  field is missing, add it as `manifest: Option<ManifestIdentity>` and ensure `None`
  serializes as JSON `null`. The field must ALWAYS be present in the emitted record; do
  not use `skip_serializing_if`.
- Resolve the identity ONCE per server session, in the same place the session `Config`
  is resolved (today: `src/mcp/server.rs` `run`, line 28; the prerequisites may have
  moved this), and thread the resulting `Option<ManifestIdentity>` to wherever records
  are built, so every record of the session carries the same identity. Do not re-read
  the policy file per call, and do not add any reload or file-watching machinery.
- Every record gets the stamp: allowed, denied, and shadow-denied calls alike.

#### Branch B: the audit subsystem does NOT exist

- Do not create `src/audit/`, any record type, or any destination.
- The deliverable is the `ManifestIdentity` `Serialize` derive with the pinned JSON
  shape test (section 6, test 6) and this marked integration point as a doc comment on
  `ManifestIdentity` (append to its existing doc comment):

```rust
/// INTEGRATION POINT (audit subsystem): embed as `manifest: Option<ManifestIdentity>`
/// on the audit record; None must serialize as JSON null and the field must always be
/// present (shared-format 6.1).
```

- Make no change to `src/mcp/server.rs` or `src/dispatch.rs` in this branch.

### 5. Display the identity

#### 5a. `browser-mcp doctor` (mandatory; the command exists today)

Add a formatting helper to `src/policy/identity.rs` so the section is unit-testable and
`run_doctor` stays thin:

```rust
/// Body lines of the doctor "Policy manifest:" section, each pre-indented two spaces.
pub fn manifest_section_lines(status: &ManifestStatus) -> Vec<String>
```

Exact lines per status:

- `Absent`: one line, `format!("  none (all-open)")`.
- `Active(id)`: three lines, in this order:
  - `format!("  {:<8} {}", "name", id.name)`
  - `format!("  {:<8} {}", "version", id.version)`
  - `format!("  {:<8} {}", "hash", id.hash)`
- `Invalid { path, error }`: one line,
  `format!("  {}: invalid ({}); identity unavailable", path.display(), error)`.

In `src/install/mod.rs` `run_doctor` (line 719 today), after the `MCP clients:` loop
and before the final `Ok(())` (line 756 today), print the section:

```rust
println!("\nPolicy manifest:");
for line in crate::policy::identity::manifest_section_lines(&status) {
    println!("{line}");
}
```

Where `status` comes from: in Branch B (standalone), call
`crate::policy::identity::manifest_status()`. In Branch A (G12 landed), build the same
`ManifestStatus` (or an equivalent the helper accepts) from the engine's loader, using
the sources doctor can see: the org policy file always, and the `BROWSER_MCP_MANIFEST`
env source only if the engine's loader is callable synchronously outside the server. The
`--manifest` flag is a server-role flag doctor cannot see; that is acceptable and needs
no note in the output. Doctor's existing sections, verdict, and exit behavior are
otherwise unchanged; ignore `DoctorOptions.verbose` (the section is always printed).

#### 5b. `config list` (conditional)

If a `config list` CLI command exists when G09 runs (it is owned by the
configuration-registry task; it does NOT exist at authoring time), append the identity
to its output as one trailing block, exactly:

- Manifest active: `Manifest: <name> <version> <hash>` (single spaces between the three
  values).
- No manifest: `Manifest: none (all-open)`.
- Invalid org policy file: `Manifest: invalid (<error>)`.

If the command does not exist, do NOT create it; the doctor section satisfies the
display requirement, and this sub-item is skipped without substitute work.

#### 5c. `get_status` (conditional, alignment only)

If the native-messaging settings handler (shared-format section 9) exists when G09
runs, its `governance.manifest` object must be produced by serializing the same
`ManifestIdentity` (never a second hand-built shape). If it does not exist, do nothing
here.

### 6. Tests

Inline `#[cfg(test)] mod tests` in `src/policy/identity.rs` (pure, no file I/O except
where noted). Required tests and assertions:

1. `canonical_hash_is_whitespace_and_bom_insensitive`: these three sources all hash to
   exactly `8f834113c263e4430f86580b7be7b14248fa65686eaf81ff60965c91a809ba90`
   (independently computed for this prompt):
   - the compact bytes `{"name":"a","version":"1","grants":[]}`;
   - a reformatted version of the same document with spaces, LF and CRLF line breaks
     between tokens;
   - the compact bytes prefixed with the UTF-8 BOM `\xEF\xBB\xBF`.
2. `canonical_hash_is_sensitive_to_key_order_and_content`:
   `{"version":"1","name":"a","grants":[]}` (reordered keys) and
   `{"name":"b","version":"1","grants":[]}` (changed content) each produce a hash
   DIFFERENT from the test-1 hash (assert inequality; do not pin their values).
3. `canonical_hash_of_the_empty_object`: `{}` hashes to exactly
   `44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a` (pins the SHA-256
   wiring and the hex rendering against an independent vector).
4. `hash_is_64_lowercase_hex`: the test-1 hash has length 64 and every char is in
   `0-9a-f`.
5. `identity_extraction_requires_name_and_version`:
   - a valid manifest source yields the exact `name`, `version`, and 64-hex `hash`;
   - `{"version":"1"}` errors with the message naming `name`;
   - `{"name":"a","version":2}` errors with the message naming `version`;
   - `[]` errors as not-an-object;
   - `not json` errors as invalid JSON.
6. `identity_serializes_as_the_audit_manifest_object`:
   `serde_json::to_string(&ManifestIdentity { name: "acme-clinical-pilot".into(),
   version: "2026.07.1".into(), hash: <the test-1 hash>.into() })` equals exactly
   `{"name":"acme-clinical-pilot","version":"2026.07.1","hash":"8f834113c263e4430f86580b7be7b14248fa65686eaf81ff60965c91a809ba90"}`.
7. `manifest_section_lines_render_each_status`: `Absent` yields exactly
   `["  none (all-open)"]`; `Active` yields the three `name`/`version`/`hash` lines with
   the `{:<8}` padding of section 5a; `Invalid` yields the single `invalid (...);
   identity unavailable` line containing the path and the error text.
8. Branch B only, `manifest_status_reads_the_org_policy_file`: using a temp directory
   (std only; e.g. a subdirectory of `std::env::temp_dir()` cleaned up at test end) and
   a small refactor seam such as a private `fn status_at(path: &Path) -> ManifestStatus`
   that `manifest_status()` delegates to: a missing file yields `Absent`; a valid
   manifest file yields `Active` with the expected triple; an invalid-JSON file yields
   `Invalid` whose error mentions JSON. Test the seam, not the real platform path.
9. Branch A (audit) only, on the audit record: a record built with no active manifest
   serializes with `"manifest":null`; one built with an identity serializes with the
   exact three-key object of test 6. Put this wherever the audit task keeps its record
   tests.

All pre-existing tests, including `tests/tool_schema_fidelity.rs` and
`tests/mcp_protocol.rs`, pass unchanged.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or descriptions.
   `tests/tool_schema_fidelity.rs` must pass unchanged. This task does not touch the
   tool surface at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task changes NO extension file; identity lives entirely in the
   binary.
3. All-open stays first-class: with no manifest and default config, behavior is
   byte-identical to today (SPEC sec 5.3 STEP 0 short-circuits to Allow). G09 preserves
   this trivially: it enforces nothing, denies nothing, and changes no dispatch or tool
   code path. Absence of the org policy file is normal, silent, and means identity is
   simply `null`/absent everywhere.
4. ASCII only in ALL code and docs: no em-dashes, arrows, or curly quotes anywhere,
   including comments, error messages, and doctor output. Use ` -- ` (double hyphen)
   where the codebase uses it.
5. The engine is truthful: a present-but-invalid org policy file is reported plainly
   (the `Invalid` doctor line and one `tracing::warn!`), never silently swallowed; and
   nothing in this task claims protection (identity is attribution, not enforcement).
6. `sha2 = "0.10"` is the ONLY new dependency permitted (sanctioned by the shared format
   doc's crate note). No `hex`, no `chrono`, no `uuid`, no dev-dependencies. Do not
   remove or alter the `preserve_order` feature on `serde_json`; the canonical hash
   depends on it.
7. Rust 2021 edition; `thiserror` for the error enum; doc comments on every public item
   and a module doc comment on the new module; `cargo fmt` clean;
   `cargo clippy --all-targets -- -D warnings` clean. Unit tests inline; integration
   tests (if any) under `tests/`.
8. Do NOT copy code from other projects; implement from the behavior described here.

Task-specific:

9. The hash is computed, never stored: this task never writes to the org policy file or
   any manifest source (read-only access), and never emits the hash into a manifest.
10. Hash the canonical bytes (BOM-strip, parse, compact re-serialize), NEVER the raw
    file bytes. Parse exactly once per source; hash and field extraction share the one
    parsed value.
11. Identity resolves once per process/session. No file watching, no mid-session reload,
    no caching layer beyond holding the resolved value.
12. One shape, one type: everywhere the identity appears (audit record, doctor, config
    list, get_status), it derives from `ManifestIdentity`; never hand-build a second
    `{name, version, hash}` shape.
13. Follow exactly one branch in sections 3 and 4 based on what has actually landed;
    re-read the tree first. If G12 landed, do not build the standalone reader. If audit
    has not landed, do not build any audit machinery.

## Verification

1. From the repo root: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   and `cargo test` are all clean and green. `tests/tool_schema_fidelity.rs` and
   `tests/mcp_protocol.rs` pass without any edit.
2. Grep the changed/new files for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/policy/identity.rs src/install/mod.rs`); there must be
   none.
3. Build note: if `target/debug/browser-mcp.exe` is locked by a running session, rename
   it aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and
   rebuild. Binary changes need an MCP client restart to observe live; no extension
   reload is relevant (no extension change).
4. Manual doctor check (Windows wording; adapt paths per shared-format 1.2 elsewhere):
   - With no `%ProgramData%\browser-mcp\policy.json`, run `browser-mcp doctor`: the
     output ends with a `Policy manifest:` section whose body is `  none (all-open)`.
   - Create `%ProgramData%\browser-mcp\policy.json` containing the shared-format 4.1
     example manifest. Run doctor again: the section shows `name`, `version`, and a
     64-char lowercase hex `hash`.
   - Re-save the same file reformatted (different indentation, CRLF vs LF): the hash is
     UNCHANGED. Change one value (e.g. `version`): the hash CHANGES.
   - Replace the file content with `not json`: doctor shows the
     `invalid (...); identity unavailable` line and the binary still exits normally.
   - Delete the file to restore the all-open state.
5. If the audit subsystem landed (section 4 Branch A): with the policy file present,
   start a session, make any tool call, and confirm the audit JSONL line contains
   `"manifest":{"name":...,"version":...,"hash":...}` with the same hash doctor shows.
   Delete the file, restart the session, repeat: the line contains `"manifest":null`.
6. If `config list` exists (section 5b): confirm the `Manifest:` line matches the doctor
   facts for both the present and absent file cases.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Grant evaluation, domain matching, tool/access checks, enforcement, denials, or
  denial-message text. Identity attributes decisions; it makes none. (The denial-id
  formula of shared-format 7.1 CONSUMES the manifest hash, but building denial ids
  belongs to the denial-format task, not here.)
- Manifest signing, signature verification, or any authenticity/trust mechanism
  (excluded by SPEC section 10; shared-format 1.2 is explicit that ACLs plus the
  deployment channel are the guard). The SHA-256 here is attribution only.
- Full manifest validation: the `schema` version check, `grants` shape, `config`
  entries, `mode`, `identity` block, unknown-field rejection. All of that is the
  manifest engine (G12).
- Parsing `--manifest`, `BROWSER_MCP_MANIFEST`, or `env://` sources in the standalone
  branch, and any implementation of the shared-format 1.3 selection rule. G12 owns
  source selection.
- Creating `src/audit/`, any audit record type, destination, or JSON Lines writer when
  the audit subsystem has not landed (section 4 Branch B delivers the type and the
  marked seam only).
- Creating the `config list` command, the `policy explain` / `policy simulate`
  commands, JSON Schema generation, or the native-messaging `get_status` /
  `get_config` / `set_config_key` handlers. Other stage-2 tasks own those; G09 only
  feeds them one type.
- Identity for the USER config file (shared-format 1.1). It is a settings file, not a
  manifest; it has no name/version and gets no hash.
- Mid-session manifest reload, file watching, or hash re-computation per call.
- Any change under `extension/`, any change to `src/mcp/schemas/tools.json`, and any
  change to `docs/SPEC.md` (the SPEC amendments are tracked in the shared format doc's
  "SPEC updates needed" list and are a separate docs task).
- Any new dependency beyond `sha2` (constraint 6), including dev-dependencies.
