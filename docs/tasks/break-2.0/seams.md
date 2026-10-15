# Break 2.0: seams ledger

The seams lane carries the 2.0 decisions of [ADR-0199](../../adr/0199-one-service-thin-shells.md)
in its Migration order: the removals, the catalog golden, policy schema 4, audit v2, the
refresher, private files on Linux, the protocol-4 trims, relay protocol 4, the edge and the
carriers, the journeys, the pristine pass, the guards and the version, and the current-state
documents. The ground rules are the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md) plus the gate
below. The switch ledger's remaining slices ([switch.md](switch.md), switch-8 onward) continue
here. The wire is [adapter-protocol-4](../../design/adapter-protocol-4.md); ADR-0199 wins over
ADR-0197 where they differ.

Gate, from the repo root on `dev`, in Git Bash, with `CARGO_TARGET_DIR=.target-lane-seams`:

- `cargo fmt --check` (fix only with `cargo fmt -p <package>` for each package a slice changed,
  never a bare `cargo fmt`), `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`.
- A Linux compile check of frozen code: `cargo clippy -p ghostlight-relay --target
  x86_64-unknown-linux-gnu --all-targets -- -D warnings` when `crates/relay` changed and, from
  slice f08a on, the same for `-p ghostlight` when `crates/edge` changed.
- `npm test` in `extension/` when `extension/` or `tests/golden/` changed.
- `node --test "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js"`
  when page assets changed.
- `node --test tests/guards/` when `crates/relay`, `crates/edge`, a connector crate, any
  `Cargo.toml`, `Cargo.lock` or `tests/golden` changed.
- `npm test --prefix packaging/npm` and `node --test packaging/mcpb/test/launcher.test.js` when
  `packaging/` changed.
- `sh -n` on every changed `scripts/*.sh`, a parse-only check of every changed `.ps1`, and
  `node --check` on every changed `.js` and `.mjs` file, journeys included.
- `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`.

Run `rustup target add x86_64-unknown-linux-gnu` once before the first Linux compile check. The
coordinator's host has only the Windows target and CI runs only after the owner pushes.

Must not edit: `CHANGELOG.md` and `docs/STATUS.md` (the coordinator owns them);
`crates/desktop/**` and any spike directory the desktop shell track names; `server.json` (it
tracks the published release); `coordination/**`; `dist/**`; `site/**`; `open-spec/**`;
`reference/**` (clean-room study material, read-only); `local/`, `private/` and `saps/` (never
read them either); the untracked notes `docs/testing/independent-user-delight-2026-10-02.md` and
`docs/testing/quiet-success-local-deployment-2026-10-02.md` (never stage them).

Rules:

1. Before each slice read `AGENTS.md`, the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md), this
   ledger and every ADR the slice names. ADR-0199 wins over any blueprint wording a spec quotes.
2. Never run any Ghostlight binary, journey, install, carrier, `deployment install` or
   `scripts/dev-loop.ps1`. The coordinator runs every live check a spec names; record each under
   Owed live checks. `cargo test` may start ordinary OS programs (`ping`, `sleep`) where a spec
   says so. Read-only check scripts (`scripts/adapter-compatibility.ps1`,
   `scripts/check-public-surfaces.ps1`) may run.
3. One slice is one commit unless the spec names a short run, with the conventional-commit subject
   given. Stage and commit only your own paths: `git add -- <paths>`, then `git commit -s -m
   '<subject>' -m '<body>' -m 'Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>' --
   <paths>`, so another lane's staged files stay out. Retry while `.git/index.lock` exists. Never
   `git add -A`, `git add .`, `git commit -a`, stash, reset, `checkout --`, clean, rebase, push or
   tag.
4. Update this ledger in the same commit: a Landed row, RESUME HERE (the next slice key), owed
   live checks, and every place the tree differed from the spec.
5. Line numbers in specs drift: re-find code by symbol. When the tree contradicts a spec, follow
   the tree, keep the spec's intent and record the difference here. When the intent cannot be
   kept, stop and record a blocker here instead of improvising.
6. Code style per `AGENTS.md`: ASCII only; a module doc comment on every module and a doc comment
   on every public item; no magic strings (closed vocabularies are enums or named consts beside
   their use); every model-facing or person-facing sentence lives in
   `crates/orchestrator/src/language/`; no comments that narrate history; lint suppressions are
   `#[expect(lint, reason = "...")]`; prefer the root fix.
7. Tests are behavioral: they drive real functions with fakes or temp homes and never pin source
   text. New unit tests of `crates/relay` and `crates/edge` go in `src/<module>/tests.rs` files
   (declared `#[cfg(test)] mod tests;`) or under `crates/<crate>/tests/`, never inline in a src
   file, so the frozen source lock (slice f12a) can exclude them. Existing inline test modules
   move when their module is rewritten, and slice f12a moves the rest. A test that needs a home
   uses `GHOSTLIGHT_TEST_HOME` or a temp directory, never the real `~/.ghostlight`.
8. Until slice f12a2-asset-pin drops the pin, a page-asset change refreshes
   `tests/golden/protocol4/assets.sha256` with the id the failing `assets/tests/world.test.js`
   prints.
9. Commit `Cargo.lock` with every dependency change. When a relay, connector or edge dependency
   changes, regenerate `tests/golden/cargo-tree` with `GHOSTLIGHT_GUARD_UPDATE=1 node --test
   tests/guards/` and say why in the commit body.
10. Deleting a tracked file listed in `docs/repository-integrity-baseline.json` also removes its
    entry.
11. Product identity is inherited: never change icons, the native host name
    `org.sylin.ghostlight`, the home `~/.ghostlight` or public wording beyond what a spec says.
    Docs follow `docs/MEMORY.md` ("Delight is clarity").
12. Root manifest rule: keep `crates/desktop` in `[workspace]` members and the `ghostlight-desktop`
    `[[package]]` block in `Cargo.lock`. Never remove any of its dependencies: `crates/desktop`
    belongs to the shell-spike lane, and its manifest declares crates it uses only in later spike
    slices.
13. The shell-spike lane runs in parallel in the same checkout. A gate failure inside
    `crates/desktop/**` or `docs/tasks/shell-spike/**` (fmt, clippy, tests, the guards, or the
    integrity script's local links) is that lane's work in progress: wait two minutes and retry;
    after 15 minutes record a blocker here and stop. Never fix, format or stage it.
14. Every slice that changes `crates/relay` or `crates/edge` records under Owed live checks:
    `cargo test -p ghostlight-relay` (and, from f08a on, `cargo test -p ghostlight` for the edge)
    on a Linux test machine.
15. Owner question Q1 (the install-time page) is answered in Notes below; f06e applies the
    recorded branch.

Hard order: shell-spike-1 commits before f01a starts (f00 is docs only and may run at the same
time). f01a's precondition checks it.

## RESUME HERE

Next: f03a-attention-state.

## Landed

| Slice | What | Commit |
|---|---|---|
| f00-adrs | ADR-0199 is filed and indexed with Decisions 1 to 23 and the owner's 2026-10-15 rulings (2.0 waits for the frozen set only, the shell ships in the service's build and is not frozen, the extension opens its setup page only when the service is absent, one-shot commands in 2.0). ADR-0198 gains its dispositions amendment. The ADR index gains the 0199 row and the status notes of the ADRs 0199 touches. This ledger starts, and switch.md points here. | see `git log` |
| f01a-flatpak | The Flatpak route and its D-Bus activation are gone (ADR-0197 D22, ADR-0165 superseded): `install/flatpak.rs`, `bridge/src/desktop_activation.rs`, `LaunchMode::FlatpakNativeHost` with its parsing and `run_flatpak_native_host`, `desktop_bus_start` and its `DBUS_STARTER_BUS_TYPE` branch, the desktop's activation-name claim, and bridge's Linux dependencies (`zbus`, `async-io`, `futures-lite`). `native-host ... --flatpak-chromium` is an ordinary usage error. The break migration gains one leftover step, `retire_flatpak_route`: it removes the Flatpak manifest and the D-Bus registration only when their content names Ghostlight, restores the override file only when it still holds the record's `permission_after`, then deletes the custody record, writes no grant, and reports each step. | see `git log` |
| f01b-peer-observation | Observed peer identity is gone (ADR-0197 D22, ADR-0161 stage 2 superseded). `Attribution` keeps `connection_id`, `observed_at_ms` and `channel`; `PeerObservation`, `SignatureStatus`, `observe`, `with_peer`, `observed_executable`, `AuditRecord.peer_image` and `with_peer_image` are removed, `ConnectionEvidence::new(label, channel)` replaces them and `Attribution::legacy` takes only the channel. `Attribution` no longer denies unknown fields, so lines that carry `provenance.peer`, `provenance.signature` or a top-level `peer_image` still read. `ConnectionDetails` keeps the connection id, the live reported application, the channel and one explanation sentence per case, authored in `language/provenance.rs`; the workbench's observation and signature rows and "Observed executable" are gone. `ghostlight-win-peer` loses `table.rs`, `image.rs`, `identify_connection`, `identify_addresses`, `PeerIdentity` and their tests, and its module doc describes what remains. `tests/provenance-journey.mjs` keeps its identity and channel checks. ARCHITECTURE and ACCEPTANCE no longer claim observed executables. | see `git log` |
| f01c-yaml-copy-setup | goose and Continue get Copy setup instead of a YAML editor (ADR-0197 D22). `ConfigDialect::Manual(ManualDialect::{Goose, Continue})` replaces the YAML dialect: the file is never read, parsed or written; the state is NotDetected or Available from the existing presence evidence; `can_install`, `can_fix` and `can_uninstall` are false; the detail is one sentence naming the file; `setup_detected` skips it; an explicit install, fix or uninstall returns `HarnessError::ManualSetup` with that sentence and changes nothing. All YAML parse and edit functions, their tests and `serde_yaml` (with `unsafe-libyaml` and `ryu`) are gone. README and the installation guide say goose and Continue use Copy setup. | see `git log` |
| f02-catalog-golden | The model surface is pinned (ADR-0197 D8 and D17, ADR-0198 D26). `ToolDefinition.output_schema`, `outcome_schema()`, the strip in `render_catalog`, `listed_tools()` and the tests that only read the output schema are gone; `tools_list_carries_no_output_schema` stays. `docs/1.0/catalog.json` is the exact `tools` array of `tools/list`, pretty-printed; `the_catalog_matches_docs_1_0_catalog_json` and `the_catalog_golden_is_ascii` guard it. `tests/golden/model/success.json` and `refusal.json` pin the complete `tools/call` result (content, structuredContent, isError) at the latest MCP revision through the real session and `lean::project`, with volatile ids normalized; `a_listed_tab_matches_the_success_golden` and `a_call_that_does_not_decode_matches_the_refusal_golden` (in `work/tests/golden.rs`) guard them, and `GHOSTLIGHT_CATALOG_UPDATE=1` rewrites all three. These two files are the envelope pin. ACCEPTANCE, ARCHITECTURE and LANGUAGE stop promising an output schema; AGENTS.md names the regeneration command. | see `git log` |

## Owed live checks

The coordinator runs every live check a slice names and records it here; implementers never run
them.

- f01a: on a Linux host whose home holds a 1.x Flatpak route (the Flatpak Chromium manifest, the
  D-Bus service file, the override grant and `flatpak-activation.json`), `deployment install`
  removes or restores each file and prints one line per file; a second install prints none.
  `cargo clippy -p ghostlight --target x86_64-unknown-linux-gnu` was not run (the orchestrator
  needs the GTK and WebKit system libraries); the removed Linux-only code left no references.
- f01b: on Windows, `node tests/provenance-journey.mjs` against a built tree (it starts the
  service, so only the coordinator runs it) passes with no executable or signature in any
  receipt's provenance, and `tests/workbench-history-browser.mjs` passes against the new
  connection fixtures. `node tests/workbench-surface.mjs` was run and passes (it starts no
  Ghostlight program).
- f02: `node tests/process-journey.mjs` and `node tests/live-journey.mjs` pass their `tools/list`
  assertion (no `outputSchema`, every tool annotated); both files pass `node --check`.

## Notes

- Owner question Q1 (the install-time page), answered 2026-10-15 by the coordinator under the
  owner-approved summary of blueprint decision 3: the frozen extension opens its own bundled
  `setup.html` (`chrome.runtime.getURL('setup.html')`), not the sylin.org post-install page, when
  its first connection finds the service absent. sylin.org stays for documentation and the setup
  page links to it. ADR-0199 Decision 23 records the bundled branch only; f06e applies it.
- Owner question Q2, answered the same day: the setup page shows the get-script one-liners
  (Windows: `irm <main>/scripts/get.ps1 | iex`; Linux: the `get.sh` line), with no Node.js
  requirement. The install journey's ADR records it.
- f01a: the tree differed from the spec in four places. (1) No Linux journey under `tests/linux`
  exercised the route, so none changed. (2) `fs2` was used only by `install/flatpak.rs`; it is
  dropped from the orchestrator and the workspace manifest. `zbus`, `async-io` and `futures-lite`
  stay in `Cargo.lock` through `tauri` on Linux, but no Ghostlight crate depends on them. (3) The
  leftover cleanup is planned only when a file is Ghostlight's own (a manifest naming the host, a
  registration starting a `ghostlight` executable, a readable custody record), so a person's
  foreign file never makes every install a migration; a file that changes between the plan and
  the step is left and reported. A failed step keeps the custody record so the next install tries
  again. (4) `native_host::manifest_bytes` and `owned_manifest_connector` served only the route;
  they are replaced by `names_ghostlight_host`. The leftover sentences print from
  `cli/deployment.rs` beside the other install lines, not from `language/`, which holds no
  install sentences.
- Where ADR-0199's draft differed from the approved text, the ADR as filed wins: the edge has a
  carrier check and three rules and knows no verb, and 2.0.0 waits for the frozen set only.
- f01b: the tree differed from the spec in four places. (1) `image.rs` could not simply go:
  `ancestor_images` (the installer's container check) uses `full_image_path`, so that one
  function moved into `process.rs` on its existing handle guard and `image.rs`, with its
  `bounded_name`, is deleted. (2) `docs/trust/*.md` and `docs/guides/siem-integration.md` made no
  peer claim and the audit-line example never listed `peer_image`, so only `docs/1.0/ARCHITECTURE.md`
  and `docs/1.0/ACCEPTANCE.md` changed; ADR-0161 already carries the supersession note. (3) The
  workbench's client label no longer falls back to an observed executable when a restored receipt
  has no reported application; it shows the short workspace id, like any other missing claim.
  (4) The browser fixtures `tests/workbench-preview-server.mjs`, `workbench-surface.mjs` and
  `workbench-history-browser.mjs` carried the removed row and now carry the new explanation
  sentences. `ghostlight-mcp-connector`'s `a_held_request_left_by_a_service_that_died_mid_flush_still_starts_one`
  failed once under a full-workspace test load and passed on every rerun; it is timing-sensitive
  and untouched by this slice.
- f01c: the tree differed from the spec in three places. (1) The person-facing sentence is authored in `install/mod.rs` beside the other harness details (the spec's wording), not in `language/`, which holds none of them. (2) `tests/workbench-preview-server.mjs` still shows Continue as `updatable` in its fixture; no test depends on it and it was left. (3) `node --test tests/guards/` needs the glob `"tests/guards/*.test.mjs"` on Node 24.
- f02: the tree differed from the spec in three places. (1) Both result goldens come from the real executor with the fake browser (`work/tests/golden.rs`): the success is a `browser_tabs` list after a `browser_navigate`, the refusal is a `browser_tabs` call with an unknown action, so the pinned envelope carries the `detail` fact and the decode error's guidance exactly as `work/mod.rs` sends them. The shared compare-or-rewrite helpers live in `language/self_check/golden.rs`. (2) The resume token never appears in a `tools/call` result, so the normalizer lists only the per-run keys (`invocation`, `tab`, `resume`, `at`, `timestamp`). (3) `tests/process-journey.mjs` and `tests/live-journey.mjs` asserted that every listed tool advertises an `outputSchema`; both now assert it is absent, as `tests/installed-journey.mjs` does.
