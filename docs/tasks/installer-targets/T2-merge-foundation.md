# T2: merge dialects + JSONC-safe fallback (foundation for Zed/OpenCode/Crush)

**Goal.** Teach the installer's JSON layer three new dialects (`context_servers`, OpenCode's `mcp`,
Crush's `mcp`) and make a JSON parse-failure (a JSONC config with comments) degrade to a printed
manual step instead of a hard error. No client is wired up yet -- this task is pure substrate and
leaves the tree green with the new dialects unused. T3-T5 consume it.

Normative: ADR-0071 D2/D3 and `PINS.md` P2/P3/P4 (exact code + oracles). Transcribe them verbatim.

## Tree facts (AS OF AUTHORING 2026-07-13 -- RE-READ before editing)

- `crates/core/src/install/merge.rs`: `pub enum Dialect { McpServers, Servers }`; `top_key(self)`;
  `impl ServerEntry { pub fn to_value(&self, dialect: Dialect) -> Value { ... } }` (the current body
  special-cases `Dialect::Servers` to add `type:"stdio"`); `pub enum MergeError { NotAnObject,
  KeyNotObject(&'static str), Parse(String) }`; a `#[cfg(test)] mod tests` with `fn entry()`,
  `fn parse(&str) -> Value`, and dialect tests including `servers_dialect_adds_type_stdio`.
- `crates/core/src/install/mod.rs`: `plan_client_install`'s `AddVia::JsonFileMerge(dialect)` arm
  ends with `match merge::merge_server(&existing, dialect, entry).and_then(|_| merge::server_matches(
  &existing, dialect, entry)) { Ok(noop) => Action {...}, Err(e) => blocked(label, target,
  e.to_string(), manual) }`. `Op::Manual` and `Tally { manual, failed, .. }` exist.
- `crates/core/src/install/clients.rs`: `server_registered`'s `AddVia::JsonFileMerge(dialect)` arm is
  `merge::has_server(contents, dialect, name).unwrap_or(false)`.

**STOP preconditions.** STOP and mark BLOCKED if: `Dialect` is not the two-variant enum above;
`to_value` no longer branches on dialect; `MergeError::Parse(String)` is gone or renamed; the
`JsonFileMerge` arms in mod.rs/clients.rs differ structurally from the above.

## Edits (all pinned in PINS.md -- transcribe)

1. `merge.rs`: add the three `Dialect` variants (PINS P2.1), extend `top_key` (P2.2), replace
   `to_value`'s body (P2.3). Add the three merge tests (P2.4).
2. `mod.rs`: split the `JsonFileMerge` arm's `Err(e)` into `Err(merge::MergeError::Parse(_)) =>
   Op::Manual` and `Err(e) => blocked(...)` (PINS P3).
3. `clients.rs`: add the substring fallback to `server_registered`'s `JsonFileMerge` arm and add the
   `jsonc_config_with_comments_is_detected_by_substring_fallback` test (PINS P4).

Nothing else. Do NOT add any `ClientId` or client wiring here (that is T3-T5).

## Verify

```
CARGO_TARGET_DIR=target-check cargo fmt --check
CARGO_TARGET_DIR=target-check cargo clippy -p ghostlight-core --all-targets -- -D warnings
CARGO_TARGET_DIR=target-check cargo test -p ghostlight-core --lib install::
```
All green; the four new tests (three in `merge::tests`, one in `clients::tests`) pass. If
`fmt --check` flags whitespace, run `cargo fmt` once (formatting is rustfmt's; the logic is the
oracle).

## Out of scope

- No `ClientId`/`CLIENTS`/`config_path`/`detect` changes (T3-T5).
- No new dependency. The JSONC handling is parse-failure routing + substring detection only.
- No manual-hint enrichment beyond what PINS P3 permits.

## Commit

`feat(install): add context_servers/opencode/crush merge dialects + JSONC-safe fallback (ADR-0071)`.
Then update the LEDGER RESUME HERE to point at T3.
