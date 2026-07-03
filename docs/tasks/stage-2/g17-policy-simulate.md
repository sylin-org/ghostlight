# G17: policy simulate (replay audit JSONL against a candidate manifest)

## Goal

Add `browser-mcp policy simulate <MANIFEST> --replay <AUDIT_JSONL>`: a read-only CLI
command that parses recorded audit records, re-evaluates each one through the SAME pure
decision function live enforcement uses (no parallel logic anywhere), and prints a plain
ASCII report with stable ordering: total actions, would-allow count, a would-deny list
grouped by (grant, domain, tool) with counts and denial ids, and every record it could
not evaluate (version-skewed or malformed) counted honestly instead of skipped silently.
Exit code 0 when there are zero would-denies, 2 when at least one exists, so CI can gate
policy changes on recorded reality.

This is ADR-0020 commitment 3: because the audit flight recorder ships before
enforcement (ADR-0018), an organization baselines real agent traffic in observe mode,
then tests a candidate manifest against actual usage instead of guessing what will
break. Simulate and live enforcement calling one decision function is what makes the
preview unable to lie about behavior (ADR-0020 consequences).

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file location, enum value, rule string, and id format in this task comes
  from it verbatim. Read it before writing any code. Load-bearing sections here: 4
  (manifest shape, content hash, grants), 5 (domain pattern matching), 6 (audit record
  fields: `tool`, `action`, `rw`, `domain`, `decision`, `grant_id`, `denial_id`,
  `manifest`), 7 (denial id and rule strings), 8 (read/write classification).
- G05 (`src/policy/classify.rs`): `pub fn classify(tool: &str, action: Option<&str>)
  -> Option<RwClass>`. Simulate uses it to recompute each record's class. If
  `src/policy/classify.rs` does not exist, stop and land G05 first.
- The stage-2 manifest task: it defines the `Manifest` type, the loader/validator
  (schema 2, required `name` / `version` / `grants`, grant validation), and the
  section-4.2 content hash. Simulate loads the candidate manifest with that machinery;
  it does not parse manifests itself.
- The stage-2 enforcement task: it produces the pure per-call decision function (the
  thing `policy_check` at the dispatch chokepoint calls), which takes the active
  manifest, the sacred-domain list, and the call facts (tool, action, current host) and
  returns either allow or a structured denial carrying `grant_id`, the `rule` string,
  and the stable `denial_id` (shared-format section 7). Simulate calls this exact
  function. If it does not exist yet, stop and land the prerequisite; do not invent a
  second evaluator here.
- The stage-2 audit-subsystem task: it defines the JSON Lines record of shared-format
  section 6 that this command reads. Simulate must reuse its record shape (and its
  serde types if they deserialize cleanly from a single line); it never writes records.
- The policy-explain task, only if it has already landed a `Policy` subcommand in
  `src/main.rs`: extend that subcommand with `simulate` instead of adding a second one.

Because those prerequisites reshape `src/policy/`, `src/dispatch.rs`, `src/main.rs`,
and create `src/audit/` before G17 runs, the "Current behavior" section below records
the tree as it stands at authoring time. Do NOT trust it as the state you will edit.
Re-read every file named below and integrate against the code the prerequisites
actually produced.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

    MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser

The two binary roles run as separate OS processes bridged by tokio-native named-pipe
(Windows) / Unix-domain-socket IPC.

Stage 1 (docs/tasks/release-1/) hardened the engine. This is stage 2, the governance
layer: a separable overlay (ADR-0013; all-open stays first-class), landed in
observe-then-enforce order (ADR-0018), configured through a typed key registry
(ADR-0019), with the org policy experience of ADR-0020 (generated schema, explain,
simulate, shadow, manifest identity, structured denials). Authority order: where
`docs/SPEC.md` and the ADRs disagree, the ADRs win; the shared format doc is the
reconciled single source for names and formats.

G17 is a pure consumer task: it reads a manifest (manifest task), reads audit records
(audit task), classifies calls (G05), and asks the one decision function (enforcement
task) what enforce mode would do. Its own contribution is the replay loop, the honest
accounting, the report rendering, and the CLI wiring.

## Current behavior

All facts verified against the working tree at authoring time (pre-prerequisites).

- `src/policy/` contains exactly two files: `mod.rs` (the key registry seed: `KeyDef`,
  `KEYS`, `Config` with the single `content.security.secrets.redact` key) and
  `redact.rs`. There is no `simulate` module, no `Manifest` type, no decision function.
- `src/audit/` does not exist. The audit prerequisite creates it and defines the
  section-6 record.
- `src/dispatch.rs` (30 lines) is the no-op seam: `PolicyDecision` has the single
  `Allow` variant (lines 13-17), `policy_check` always allows (lines 23-25), `audit`
  does nothing (line 30). The enforcement prerequisite replaces these.
- `src/mcp/server.rs`: `handle_tools_call` (lines 116-155) calls the no-op seams at
  lines 132-133. G17 does not modify this path; it only shares the decision function
  the enforcement task threads through it.
- `src/main.rs`: `Cli` (lines 26-41) has `command: Option<Command>` plus the
  server-role `--manifest` and `--debug` flags; `Command` (lines 43-53) has exactly
  `Install`, `Uninstall`, `Doctor`, `Status`. There is no `policy` subcommand. The
  installer subcommands run synchronously with no tokio runtime; `policy simulate`
  follows that pattern.
- `tests/mcp_protocol.rs` shows the established integration-test pattern for running
  the binary: `Command::new(env!("CARGO_BIN_EXE_browser-mcp"))`.
- `examples/` does NOT exist in the tree (CLAUDE.md describes it aspirationally). See
  Required behavior item 6 for the conditional test.
- `Cargo.toml` dependencies today: `tokio`, `serde`, `serde_json` (with
  `preserve_order`), `clap`, `tracing`, `tracing-subscriber`, `thiserror`, `anyhow`,
  `dirs` (plus Windows-only `winreg`, `windows-sys`). `sha2`, `uuid`, and an RFC 3339
  time source are added by earlier stage-2 tasks (shared-format crate note). G17 adds
  no dependency of any kind.

## Required behavior

### 1. CLI surface

Add the subcommand `policy simulate` to `src/main.rs`:

    browser-mcp policy simulate <MANIFEST> --replay <AUDIT_JSONL>

- `MANIFEST`: required positional, a filesystem path to a candidate manifest JSON file
  (shared-format section 4.1). Plain path only; `file://` / `env://` source syntax is
  not accepted here.
- `--replay <AUDIT_JSONL>`: required flag, a filesystem path to an audit JSON Lines
  file (shared-format section 6). There is no default; omitting it is a clap usage
  error.
- If a `Policy` subcommand already exists (landed by the policy-explain task), add a
  `Simulate` variant to its nested subcommand enum. If not, add
  `Policy(PolicyArgs)` to `Command` where `PolicyArgs` holds
  `#[command(subcommand)] command: PolicyCommand` and `PolicyCommand` has the one
  variant `Simulate(SimulateArgs)`. `SimulateArgs` carries `manifest: PathBuf`
  (positional) and `#[arg(long, value_name = "FILE")] replay: PathBuf`.
- The command runs synchronously with no tokio runtime, like the installer
  subcommands. `main.rs` stays a thin shell: it calls one library function and maps
  the outcome to an exit code.

Exit codes:

| Code | Meaning |
|---|---|
| 0 | Simulation ran; zero would-deny events. |
| 2 | Simulation ran; one or more would-deny events. |
| 1 | Operational error: manifest unreadable or invalid, replay file unreadable. The report is not printed; the error goes to stderr. |

Implementation of the exit path: the `Simulate` arm of `main` prints the report to
stdout with `print!`, then flushes explicitly (`std::io::Write::flush` on
`std::io::stdout()`; stdout is block-buffered when piped and `std::process::exit` does
not flush it), then calls `std::process::exit(0)` or `std::process::exit(2)`. On
error, return the error from `main` (the existing `anyhow::Result<()>` return makes
the process exit 1 with the message on stderr). Do not alter clap's own usage-error
exit behavior.

### 2. Library module `src/policy/simulate.rs`

Create `src/policy/simulate.rs` and declare it with `pub mod simulate;` in
`src/policy/mod.rs`. Module doc comment: this is ADR-0020 commitment 3; it replays
recorded audit records through the same pure decision function live enforcement uses,
so the preview cannot disagree with production by construction; it also documents the
two honest limitations (records carry only the normalized host, so scheme-rule
denials and anything URL-path-dependent cannot be reproduced; simulate only covers
recorded behavior, ADR-0020 negative consequence).

Public entry point (typed errors, `thiserror`; `anyhow` stays in `main.rs`):

    /// Load the candidate manifest and the replay file, run the simulation, and
    /// render the report.
    pub fn run_simulate(manifest_path: &Path, replay_path: &Path)
        -> Result<SimulateOutcome, SimulateError>

    /// The rendered report plus the one number the exit code depends on.
    pub struct SimulateOutcome {
        /// The full plain-ASCII report (section 5 format), ready to print.
        pub report: String,
        /// Count of would-deny events (exit code 2 when nonzero).
        pub would_deny: u64,
    }

`SimulateError` is a `thiserror` enum covering: manifest file read failure, manifest
parse/validation failure (wrap the manifest task's error type), and replay file read
failure. A malformed LINE inside the replay file is NOT an error: it is a
not-evaluable record (section 4). Only failure to read the file at all is an error.

Internally, separate the pure core from I/O so it is unit-testable without files:
a function that takes the parsed `Manifest` (plus its content hash, if the manifest
task's type does not already carry it) and an iterator of `(line_number, &str)` pairs
(1-based line numbers) and returns a report struct with the counts, the deny groups,
and the not-evaluable list; and a renderer that turns that struct into the exact
section-5 text. Use `BTreeMap` keyed by the group tuple for deterministic grouping;
no HashMap iteration order may reach the output.

### 3. Reusing the one decision function (no parallel logic)

Locate the pure per-call decision function the enforcement task produced (expected
under `src/policy/`; it is whatever the dispatch chokepoint's `policy_check` calls to
turn manifest + sacred list + call facts into allow-or-denial). Simulate calls that
function, once per evaluable record, with:

- the candidate manifest loaded from `<MANIFEST>` (not any live/active manifest);
- an EMPTY sacred-domain list. The local user's `content.security.sacred_domains` is
  not part of the policy under test; a candidate manifest simulation must produce the
  same report on the admin's machine and in CI. Consequently rule `sacred` can never
  appear in a simulation report.
- the call facts reconstructed from the record: `tool`, `action`, and the recorded
  `domain` host (or the function's no-URL representation when `domain` is JSON null).
  Do not special-case the null-domain path inside simulate; pass it through and report
  whatever the decision function returns.

Mode is ignored entirely: manifest-level `mode` and per-grant `mode` fields have no
effect on simulation. Simulate always reports the enforce view (a would-deny is a
would-deny; reading would-deny events out of an observe-mode manifest before flipping
it to enforce is the whole point). If the decision function takes an effective mode or
returns a shadow variant, collapse it: any deny-shaped verdict counts as would-deny.

Forbidden: reimplementing, copying, or approximating domain matching, grant
resolution, access mapping, rule selection, or denial-id computation inside
`simulate.rs`. If the enforcement task's function turns out not to be callable as a
pure function (for example it is entangled with async or browser state), extract the
pure core minimally so both live enforcement and simulate call it, and keep live
behavior byte-identical; if that extraction is not safely possible, stop and report
rather than duplicating logic.

### 4. Record ingestion and honest accounting

Read the replay file as UTF-8 text, split on `\n` (tolerate a trailing `\r` per line:
strip one trailing `\r` if present). Number lines from 1. For each line:

- Empty or whitespace-only: skipped entirely. It is JSONL framing, not a record; it
  does not count toward any total.
- Every other line is exactly one record and lands in exactly one bucket:
  would-allow, would-deny, or not-evaluable. `total = would_allow + would_deny +
  not_evaluable` must hold by construction.

Field extraction from a parsed record (shared-format section 6.1):

| Check, in order | Bucket and reason |
|---|---|
| Line is not valid JSON, or is valid JSON but not an object | not evaluable, `malformed json` |
| `tool` key absent, or present but not a JSON string | not evaluable, `missing field: tool` |
| `domain` key absent (JSON null is fine and means no URL) | not evaluable, `missing field: domain` |
| `domain` present but neither a string nor null | not evaluable, `missing field: domain` |
| `action` read via `.get("action")`: absent or null means no action; a non-string, non-null value | not evaluable, `missing field: action` |
| `classify(tool, action)` returns `None` and `tool != "computer"` | not evaluable, `unknown tool: <tool>` |
| `classify` returns `None`, `tool == "computer"`, `action` is `None` | not evaluable, `computer action missing` |
| `classify` returns `None`, `tool == "computer"`, `action` is `Some(a)` | not evaluable, `unknown action: <a>` |
| Otherwise | evaluate via the decision function; bucket per its verdict |

Rules for the ignored fields: the recorded `rw`, `decision`, `grant_id`, `denial_id`,
`event_id`, `ts`, `duration_ms`, `identity`, `client`, and `manifest` fields are read
by nobody. Simulate replays the ACTION under the candidate manifest; it does not
compare against, trust, or validate the original decision. In particular the class is
ALWAYS recomputed via `classify` (a version-skewed recorded `rw` must not influence
the verdict), and unknown extra keys in a record are ignored (forward compatibility).

Not-evaluable records are collected as `(line_number, reason)` pairs in file order.
Nothing is ever skipped silently: a record that cannot be evaluated appears in both
the `not evaluable` total and the per-line list.

Not-evaluable records do NOT affect the exit code (the spec is: exit 2 if and only if
`would_deny > 0`); they are reported so a human or a CI grep can decide to care.

### 5. Report format (exact)

Plain ASCII, deterministic, written to stdout. `<...>` are substitutions; everything
else is literal, including spacing. Line order is exactly as shown.

Header and totals (always printed):

    policy simulate
    manifest: <name> <version> sha256=<64-hex content hash>
    replay: <replay path exactly as given on the command line>

    total actions: <N>
    would allow: <N>
    would deny: <N>
    not evaluable: <N>

- `<name>` and `<version>` are the manifest's required top-level fields; the hash is
  the shared-format 4.2 content hash of the candidate manifest, full 64 lowercase hex.
- `<N>` are plain base-10 integers, no padding, no thousands separators.

Would-deny section (printed only when `would deny` is nonzero; preceded by one blank
line):

    would-deny groups (grant, domain, tool):
    count=<N> grant=<grant_id or -> domain=<host or -> tool=<tool> rule=<rule> denial=<denial id>

One line per group. Group key: (`grant_id` or `-` when no grant matched, `domain`
host or `-` when null, tool NAME, full rule string). Notes:

- `tool` is the bare tool name from the record (`computer` stays `computer`; the
  `computer (<action>)` rendering belongs to denial MESSAGES, shared-format 7.2, not
  to this report). Grouping by tool name means all denied `computer` actions under the
  same grant, domain, and rule fold into one line; that is intended.
- `rule` is the full shared-format 7.1 rule string including any detail token (for
  example `access`, `unmatched_domain`, `tool/javascript_tool`).
- `denial` is the stable denial id (`D-` + 8 hex) exactly as the decision function
  computed it. Within one group key the denial id is necessarily constant (it is a
  function of manifest hash, grant id, and rule); do not recompute it in simulate.
- Ordering: sort group lines byte-wise ascending by the tuple
  (grant render, domain render, tool, rule). No count-based ordering.

Not-evaluable section (printed only when nonzero; preceded by one blank line):

    not evaluable:
    line <line number>: <reason>

One line per not-evaluable record, in ascending line-number order, reasons exactly as
the section-4 table spells them.

Result line (always printed last, preceded by one blank line; exact strings regardless
of grammatical number):

    result: no would-denies (exit 0)

or

    result: <N> would-denies (exit 2)

The report ends with a single trailing newline. Running the same command twice on the
same inputs must produce byte-identical output.

### 6. Tests

Fixtures (new files, authored by this task):

- `tests/fixtures/simulate/audit.jsonl` -- a hand-authored replay file whose records
  follow the shared-format 6.1 shape (any plausible `event_id`/`ts`/`duration_ms`
  values; the ignored fields may be minimal but the JSON must be valid). It must
  contain at least: an observe-class call on a granted domain; a mutate-class call on
  a domain the restrictive manifest grants read-only (yields rule `access`); several
  `computer` records with mutate actions on that same read-only domain (they must fold
  into one group line); a call on a host no grant covers (rule `unmatched_domain`); a
  call on a host covered only via a `*.` wildcard subdomain match (positive check); a
  tool blocked by `exclude_tools` under the restrictive manifest (rule
  `tool/<name>`); a record with `"domain": null`; a record with an unknown tool name
  (for example `teleport`); a `computer` record with an unknown action; a `computer`
  record with `"action": null`; one line of invalid JSON; one empty line in the
  middle of the file.
- `tests/fixtures/simulate/manifest-permissive.json` -- schema 2, `name`
  `"simulate-permissive"`, a version string, grants with `access: "all"` covering
  every granted-domain used in the fixture (list apex and `*.` wildcard patterns per
  shared-format 5.1), no `exclude_tools`.
- `tests/fixtures/simulate/manifest-restrictive.json` -- schema 2, `name`
  `"simulate-restrictive"`, one `access: "read"` grant on one fixture domain, one
  grant with `exclude_tools` on another, and no grant for the unmatched host.

Integration tests in a new `tests/policy_simulate.rs`, spawning the binary with
`Command::new(env!("CARGO_BIN_EXE_browser-mcp"))` (the `tests/mcp_protocol.rs`
pattern) and RELATIVE fixture paths (integration tests run with the crate root as the
working directory, and the report echoes the path as given):

1. Permissive manifest over the fixture: exit code 0; stdout contains
   `result: no would-denies (exit 0)`, `would deny: 0`, and NO
   `would-deny groups` section; the not-evaluable count matches the fixture's
   malformed/unknown records exactly, and each expected `line <n>: <reason>` line is
   present.
2. Restrictive manifest over the fixture (the golden test): exit code 2; assert the
   exact totals arithmetic (`total actions` equals would allow + would deny + not
   evaluable, all equal to the counts the fixture was authored to produce); assert
   each expected group line by its `count=`, `grant=`, `domain=`, `tool=`, and
   `rule=` substrings; assert every `denial=` value matches `D-` followed by 8
   lowercase hex characters; assert the group lines appear in the specified sort
   order; assert the folded `computer` group's count covers all its fixture records.
3. Determinism: run test 2's command twice and assert byte-identical stdout.
4. Operational errors: a nonexistent replay path exits 1 with a stderr message naming
   the path and no report on stdout; a manifest file containing invalid JSON exits 1;
   a structurally invalid manifest (for example a grant with an unknown tool name)
   exits 1 via the manifest task's validation error.
5. Same-logic pin (unit test inside `simulate.rs` or the enforcement module): for one
   would-deny record from the fixture scenario, call the decision function directly
   with the same manifest and call facts and assert the `denial_id` and `grant_id` it
   returns are identical to the ones the simulation report contains. This pins that
   simulate and enforcement share one code path.
6. Conditional, only if `examples/*.json` manifest files exist in the tree when you
   implement (they do not exist at authoring time): add a test that iterates them and
   asserts `policy simulate <example> --replay tests/fixtures/simulate/audit.jsonl`
   never exits 1 (0 and 2 are both acceptable). If `examples/` does not exist, do not
   create it and do not add this test.

Unit tests inline in `simulate.rs` for the pure core: empty replay input yields all
zeros and exit-0 outcome; whitespace-only lines are not counted; the section-4 bucket
table (one test per row, driving the ingestion function directly with single lines);
totals arithmetic; group sort order with `-` entries sorting first.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged. This task does not
   touch tool advertisement or schemas at all.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. This task changes no extension file; simulate lives entirely in the
   binary.
3. All-open stays first-class: with no manifest and default config, live behavior is
   byte-identical to today. G17 must not change any live enforcement, dispatch, or
   server path; it only READS shared pure functions. If Required behavior section 3
   forces a pure-core extraction, prove live behavior is unchanged by the existing
   test suite passing untouched.
4. ASCII only in ALL code and docs, including this report's output, comments, fixture
   files, and error messages: no em-dashes, arrows, or curly quotes. Use ` -- ` where
   the codebase uses it.
5. The engine is truthful: the report never overstates coverage. Not-evaluable records
   are counted and listed, never dropped; the module doc states the host-only and
   recorded-behavior-only limitations plainly.
6. No new dependencies of any kind, including dev-dependencies. `sha2` / `uuid` / the
   time source arrive via earlier stage-2 tasks; G17 adds nothing to `Cargo.toml`.
7. Rust 2021 edition; `thiserror` for the library error type (`anyhow` only in
   `main.rs` and integration tests); doc comments on every public item and a module
   doc comment on `simulate.rs`; `cargo fmt` clean; `cargo clippy --all-targets -- -D
   warnings` clean. Unit tests inline, integration tests in `tests/`.
8. Do NOT copy code from other projects; implement from the behavior described here.

Task-specific:

9. One decision function. `simulate.rs` must contain zero domain matching, zero grant
   resolution, zero access/rw mapping beyond calling `classify`, and zero denial-id
   computation. If you find yourself writing a wildcard matcher or a SHA-256 call in
   this file, stop: that logic belongs to the prerequisites and must be called, not
   copied.
10. Honest accounting is structural: every non-empty line lands in exactly one of the
    three buckets and the totals arithmetic is asserted in tests.
11. Simulate is read-only: it writes no audit records, no config, no files, makes no
    network or IPC connections, and does not require (or touch) a running mcp-server
    or the extension.
12. Mode never affects the verdict; the sacred list is empty in simulation; rule
    `sacred` cannot appear in a report.
13. The report format of section 5 is exact and stable: same inputs, byte-identical
    output, on every platform (mind CRLF: always emit `\n` only).

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green, including the new `tests/policy_simulate.rs`, the
   new inline unit tests, and `tests/tool_schema_fidelity.rs` unchanged.
4. Manual run, permissive:
   `cargo run -- policy simulate tests/fixtures/simulate/manifest-permissive.json --replay tests/fixtures/simulate/audit.jsonl`
   prints the report with `would deny: 0` and `result: no would-denies (exit 0)`;
   check the exit code (`$LASTEXITCODE` in PowerShell, `$?` in bash) is 0.
5. Manual run, restrictive: same command with
   `tests/fixtures/simulate/manifest-restrictive.json` prints the grouped would-deny
   lines with `D-xxxxxxxx` denial ids and `result: <N> would-denies (exit 2)`; exit
   code is 2.
6. Manual run, error: point `--replay` at a nonexistent file; exit code is 1, stderr
   names the path, stdout has no report.
7. Determinism: run step 5 twice and diff the outputs; they are byte-identical.
8. `git diff --stat` shows no change to `src/mcp/schemas/tools.json`, no change under
   `extension/`, and no change to `Cargo.toml` dependencies.

Build note: if `target/debug/browser-mcp.exe` is locked by a running MCP session,
rename it aside (`mv target/debug/browser-mcp.exe
target/debug/browser-mcp.exe.old-1`) and rebuild. No extension reload is needed for
this task; no MCP client restart is needed to test the CLI subcommand.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Live traffic capture, tailing, or watching the audit file. The audit subsystem
  already records live traffic; simulate reads a finished file once. No `--follow`,
  no polling, no default replay source pointing at the live audit path.
- Statistical projections of any kind: no percentages, rates, trends, time bucketing,
  sampling, confidence language, or "top N" truncation. The report is exact counts
  and exhaustive lists only.
- Any SaaS, reporting service, web console, SIEM integration, upload, or HTTP
  anything (ADR-0020 non-goals; SPEC section 10).
- Comparing the candidate's verdicts against the RECORDED decisions (old-vs-new
  policy diffing, "newly denied" / "newly allowed" deltas). Simulate reports the
  candidate manifest's verdicts, full stop.
- `policy explain`, `policy init`, manifest templates, and JSON Schema generation
  (separate stage-2 tasks). If the `Policy` subcommand does not exist yet, create
  only the `simulate` variant under it.
- Changing live enforcement semantics, the dispatch chokepoint's behavior, the audit
  record shape, or when records are written. If a minimal pure-core extraction is
  needed (section 3), it must be behavior-preserving and proven so by the untouched
  existing tests.
- Evaluating or honoring `mode` fields, the sacred-domain list, or the layered config
  registry inside the simulation (beyond loading the manifest itself).
- Filtering the replay by time range, identity, client, tool, or domain. No filter
  flags of any kind.
- Colored output, progress bars, spinners, JSON/CSV output modes, or a `--format`
  flag. One plain ASCII text format.
- Any change under `extension/`, any change to `src/mcp/schemas/tools.json`, any new
  dependency, and any edit to `docs/SPEC.md` (SPEC amendments are tracked in the
  shared format doc's "SPEC updates needed" list and belong to a docs task).
