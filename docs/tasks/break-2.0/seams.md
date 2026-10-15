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

Next: f06e (the next slice in the lane order).

## Landed

| Slice | What | Commit |
|---|---|---|
| f00-adrs | ADR-0199 is filed and indexed with Decisions 1 to 23 and the owner's 2026-10-15 rulings (2.0 waits for the frozen set only, the shell ships in the service's build and is not frozen, the extension opens its setup page only when the service is absent, one-shot commands in 2.0). ADR-0198 gains its dispositions amendment. The ADR index gains the 0199 row and the status notes of the ADRs 0199 touches. This ledger starts, and switch.md points here. | see `git log` |
| f01a-flatpak | The Flatpak route and its D-Bus activation are gone (ADR-0197 D22, ADR-0165 superseded): `install/flatpak.rs`, `bridge/src/desktop_activation.rs`, `LaunchMode::FlatpakNativeHost` with its parsing and `run_flatpak_native_host`, `desktop_bus_start` and its `DBUS_STARTER_BUS_TYPE` branch, the desktop's activation-name claim, and bridge's Linux dependencies (`zbus`, `async-io`, `futures-lite`). `native-host ... --flatpak-chromium` is an ordinary usage error. The break migration gains one leftover step, `retire_flatpak_route`: it removes the Flatpak manifest and the D-Bus registration only when their content names Ghostlight, restores the override file only when it still holds the record's `permission_after`, then deletes the custody record, writes no grant, and reports each step. | see `git log` |
| f01b-peer-observation | Observed peer identity is gone (ADR-0197 D22, ADR-0161 stage 2 superseded). `Attribution` keeps `connection_id`, `observed_at_ms` and `channel`; `PeerObservation`, `SignatureStatus`, `observe`, `with_peer`, `observed_executable`, `AuditRecord.peer_image` and `with_peer_image` are removed, `ConnectionEvidence::new(label, channel)` replaces them and `Attribution::legacy` takes only the channel. `Attribution` no longer denies unknown fields, so lines that carry `provenance.peer`, `provenance.signature` or a top-level `peer_image` still read. `ConnectionDetails` keeps the connection id, the live reported application, the channel and one explanation sentence per case, authored in `language/provenance.rs`; the workbench's observation and signature rows and "Observed executable" are gone. `ghostlight-win-peer` loses `table.rs`, `image.rs`, `identify_connection`, `identify_addresses`, `PeerIdentity` and their tests, and its module doc describes what remains. `tests/provenance-journey.mjs` keeps its identity and channel checks. ARCHITECTURE and ACCEPTANCE no longer claim observed executables. | see `git log` |
| f01c-yaml-copy-setup | goose and Continue get Copy setup instead of a YAML editor (ADR-0197 D22). `ConfigDialect::Manual(ManualDialect::{Goose, Continue})` replaces the YAML dialect: the file is never read, parsed or written; the state is NotDetected or Available from the existing presence evidence; `can_install`, `can_fix` and `can_uninstall` are false; the detail is one sentence naming the file; `setup_detected` skips it; an explicit install, fix or uninstall returns `HarnessError::ManualSetup` with that sentence and changes nothing. All YAML parse and edit functions, their tests and `serde_yaml` (with `unsafe-libyaml` and `ryu`) are gone. README and the installation guide say goose and Continue use Copy setup. | see `git log` |
| f02-catalog-golden | The model surface is pinned (ADR-0197 D8 and D17, ADR-0198 D26). `ToolDefinition.output_schema`, `outcome_schema()`, the strip in `render_catalog`, `listed_tools()` and the tests that only read the output schema are gone; `tools_list_carries_no_output_schema` stays. `docs/1.0/catalog.json` is the exact `tools` array of `tools/list`, pretty-printed; `the_catalog_matches_docs_1_0_catalog_json` and `the_catalog_golden_is_ascii` guard it. `tests/golden/model/success.json` and `refusal.json` pin the complete `tools/call` result (content, structuredContent, isError) at the latest MCP revision through the real session and `lean::project`, with volatile ids normalized; `a_listed_tab_matches_the_success_golden` and `a_call_that_does_not_decode_matches_the_refusal_golden` (in `work/tests/golden.rs`) guard them, and `GHOSTLIGHT_CATALOG_UPDATE=1` rewrites all three. These two files are the envelope pin. ACCEPTANCE, ARCHITECTURE and LANGUAGE stop promising an output schema; AGENTS.md names the regeneration command. | see `git log` |
| f03a-attention-state | The attention runtime state and its control file are gone (ADR-0197 D19). `RuntimeControls` holds active, held and ended in memory only; `GHOSTLIGHT_RUNTIME_CONTROL_FILE`, `with_runtime_control_file`, `require_attention` and `runtime_control_file_configured` are removed, so `runtime_decision` reads only the in-memory controls. `ReasonCode::RuntimeAttention`, `Status::AttentionRequired`, `Refusal::AttentionRequired`, `CheckFailure::AttentionRequired`, `AuditRefusal::AttentionRequired`, `DomainEvent::AttentionRequired` with its presentation and workbench reactions, `PresentationKind::Attention` (service and wire mirror), `RuntimeControlState::Attention`, `StepCounts.attention_required` and `EXIT_ATTENTION` are removed; the other exit codes keep their numbers. `StepCause::AttentionRequired` stays for the local interlock. `StepCounts` no longer denies unknown fields and an unknown refusal reason already reads as no refusal facts, so earlier audit lines still read. The page assets lose the attention branch and `tests/golden/protocol4/assets.sha256` is refreshed. LANGUAGE, ARCHITECTURE, ACCEPTANCE, INTENT, the scripting and SIEM guides, the adapter-protocol-4 note and the man page stop naming the state. | see `git log` |
| f03b-policy-schema4 | Policy schema 4 (ADR-0197 D19, blueprint F3). Schema 4 is schema 3 without `mode` (on the policy and on each grant), `channels.mcp.enabled`, `channels.cli.enabled`, `policy.user.enabled`, `content.frames.handling` and `content.frames.notice`; identity and the canonical hash are unchanged. `parse` accepts only schema 4 and refuses schema 3 with `unsupported policy schema 3; run ghostlight policy migrate <file>` (`ManifestError::RetiredSchema`). `governance/migrate.rs` and `language/migration.rs` carry `ghostlight policy migrate <file>`: it drops `mode` enforce, a channel set to true, the user switch and a frame option at its default, refuses everything else with one sentence per part and leaves the file untouched, and on success copies the original to `<file>.schema3.bak` and replaces the file in one step. Ghostlight's own `policy/user-policy.json` moves at load by the same rule; administrator files (the managed file and bundles, `GHOSTLIGHT_POLICY_FILE`) are never rewritten and fail closed. `PolicyMode`, `Decision.observed`, `PermissionCheck.observed`, the layer mode, `AuditRecord.policy_observed` and `policy_mode`, `admits_channel` with `ReasonCode::ChannelDenied`, `PolicyRule::Channel`, `BlockedReason::Channel` and `OpeningRefusal::ChannelNotAdmitted`, `user_authoring_allowed`, and the `DocumentPolicy` resolution (frame handling is fixed at permitted content, the notice at when affected) are gone. `PermissionCheck` and `LayerEvidence` no longer deny unknown fields, so earlier audit lines still read. The workbench editor writes schema 4 and has no watch-only, channel or frame controls. Every example moved to schema 4; `developer-observe.json` and `scripting-disabled.json` are deleted. | see `git log` |
| f04-audit-v2 | Audit line v2 (ADR-0197 D17, blueprint F4). `AuditRecord` lives in `audit.rs` beside its writer and reader (no re-export; every use imports `crate::audit::AuditRecord`). The singular `capability` field and `requirements()` are gone (readers use `capabilities`), as are `PermissionCheck.request_restricted` and `request_evaluated` with `permission()`'s restriction branches, `RETIRED_BROWSER_LANDING` with its workbench filters and test, `CompositionKind::Sequence` with the branches that read it, `Coverage.masked_regions` and `DocumentObservation.masked_regions` with their producers and the lean-view entry, and `AuditRefusal::RequestRestricted`. `read_refusal` no longer calls its facts legacy. `deny_unknown_fields` is dropped from every type an audit line reaches (`AuditRefusal`, `Coverage`, `StepReceipt`, `StepCounts` earlier, `StepIssue`, `CompositionProgress`, `EffectCounts`, `Observed`, `PermissionTrace`), so a later release's line reads and unknown fields are never written back. `tests/golden/audit/success.json`, `refusal.json` and `flow.json` pin the line (`GHOSTLIGHT_AUDIT_GOLDEN_UPDATE=1` rewrites them); one test reads each golden with an unknown field in every object and gets an equal record, another reads a line carrying every retired field through history reload, and `policy simulate` replays the success and refusal goldens. `docs/guides/siem-integration.md` shows the golden line, its field table without the removed fields, and says readers ignore unknown fields. | see `git log` |
| f05-refresher | One refresher publishes the authority and the managed HTTPS fetch leaves the call path (ADR-0197 owner amendment, blueprint F5). `GovernanceFacade` holds `published` (an immutable `Published`: the `Arc<AuthoritySnapshot>`, the diagnostics, the band chip, the compiled `EffectiveAuthority`, the startup preference and the organization layer a candidate previews under), `sources` (the two local policy files plus the last `RemoteView` of the managed authority) and `remote` (the `ManagedAuthority`, locked by the refresher alone). `snapshot()` returns `Arc<AuthoritySnapshot>` by cloning the published `Arc`; no reader reads a file or the network or takes the sources lock, and `refresh_policies` and its ten call sites are gone. `refresh_tick()` checks each policy file and the managed bootstrap by existence, modified time and size (`governance/file_state.rs`), re-reads and republishes only what changed, and runs `ManagedAuthority::refresh` when `needs_refresh()` says the bootstrap changed, the HTTPS poll is due or the local bundle changed. `refresh_local_now()` re-reads the local files with no network work; `apply_user_policy` and `remove_user_policy` call it. Construction publishes from local sources only (`ManagedAuthority::load_offline`: bootstrap, verified cache, local bundle source), so the last verified policy is enforced before the first poll. `ServiceHost` starts the thread `ghostlight-policy-refresher` (a `refresh_tick` each second, parked between ticks and woken by `join`). The per-session thread is `ghostlight-session-watch`; `AdvertisedCatalog` remembers the authority id the client's catalog was built from and rebuilds the catalog only when the id changes, sending `tools/list_changed` only when the tools differ. `RuntimeControls::observe` takes the one observer; `hold`, `resume` and `end_session` notify it once per change, in order, and `GovernanceFacade::relay_runtime_state` installs the browser port. The per-call publish in `authorize`, the workbench's publish, the extension-intent publish and the start-time publish are gone. ACCEPTANCE gains item 22 and ARCHITECTURE's governance section says the same in one paragraph. The process, frame, installed and Linux governance journeys wait `policyRefreshed()` (`tests/lib/policy-refresh.mjs`) after each in-flight policy edit. | see `git log` |
| f09-private-files | Files Ghostlight creates under the home are owner-private on Linux (blueprint F9). `ghostlight_relay::private` holds `file_options()` (mode 0600 on Unix, today's options on Windows) and `create_dir_all` (0700 on Unix). The audit sink, `install::record::atomic_replace` (and so the install record, development marker and Windows entry), the managed cache and status writes, the user-policy write and its `.schema3.bak` backup, the handoff marker, the install lock, the relay diagnostics log, the bridge diagnostics marker and logs, the relay runtime file's and service lease's directories, the executables' `bin` directory, and everything the 2.0 break migration creates in `home/policy` and `home/state` (the copies, the `.copied-sha256` records and the audit archive) all use them. The break migration copies into new private files whatever mode the 1.x original had. The managed file's and the policy migrator's private copies of atomic write are gone; both call `atomic_replace`. `relay::log`'s tests moved to `relay/src/log/tests.rs` with the new mode test. `data-flows.md` says it in one sentence. | see `git log` |
| f06a-page-message-lists | The frozen vocabulary carries only words the extension reads (blueprint F6a). `extension/lib/protocol.js` no longer lists `world_message_kinds`, `asset_error_codes` or `pre_effect_codes`; nothing in `extension/lib` or `worker.js` read them. The golden test compares `protocol.vocabulary` with `vocabulary.json` minus the named `PAGE_MESSAGE_LISTS` and asserts none is present. `vocabulary.json`, `golden.rs` and the page asset tests keep the lists: the page seam belongs to the service (ADR-0197 D6). adapter-protocol-4 says so. | see `git log` |
| f06b-tab-readiness | Protocol-4 tab readiness is loading, complete or unknown (blueprint F6b). `wire::Readiness` drops `Interactive`; a tab readiness word it does not name decodes as Unknown, and `BrowserReadiness::Interactive` maps to Loading on the wire (only the fake extension and tests send tab readiness). The page seam keeps the word: `world::DocumentReadiness` (Loading, Interactive, Complete, Unknown) decodes the observe answer and maps into `BrowserReadiness`, so model results still report interactive. `vocabulary.json`, `protocol.js`, `readiness_changed.json`, `golden.rs` and adapter-protocol-4 follow. | see `git log` |
| f06c-notice-label | The notice carries the service's activity label (blueprint F6c). `wire::NoticeSignal` gains `label: Option<String>` (default, skipped when none); the physical `PresentationSignal` carries `label: String`, set from `activity_label` for every signal the presentation reactor builds, and `translate::present` sends it. `notice.js` shows a non-empty frame label (bounded to the phase limit) in the popup's activity list and falls back to `shared.activityLabel` only when it is absent. `tests/golden/protocol4/frames/notice.json` gains `"label":"Clicking"`; adapter-protocol-4 documents the field. | see `git log` |

| f06d-setup-page | The bundled setup page and the install say only what is true in 2.0 (blueprint F6d). `extension/setup.html` drops the Debian package lines, says the command installs Ghostlight on this computer and registers this browser and the MCP clients it finds, and describes `ghostlight doctor` as this computer's side. `environment::BACKGROUND_POSTURE`, its test and its println in `finish_setup` are gone. | see `git log` |

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
- f03a: `node tests/history-compatibility.mjs` (it starts the built service, so only the coordinator
  runs it) passes without the runtime-control variable; `node tests/installed-journey.mjs` passes
  its dialog clicks, which now assert `succeeded` alone; `node tests/keyboard-browser-journey.mjs`
  (it launches Chromium) passes its passive-feedback check, which now renders a `denial` signal
  instead of the removed `attention` branch; `node tests/live-journey.mjs` records statuses
  without `attention_required`. `node tests/workbench-surface.mjs`, the page-asset tests, the
  extension tests and the guards were run and pass.
- f03b: the coordinator runs, against a built tree, `node tests/process-journey.mjs`,
  `tests/cli-journey.mjs` (its channel-refusal block is gone), `tests/frame-browser-journey.mjs`
  (Chrome/MV3; the mode and notice loops are gone), `tests/provenance-journey.mjs`,
  `tests/script-browser-journey.mjs`, `tests/local-resilience-journey.mjs`,
  `tests/fixed-installation-journey.mjs`, `tests/windows-desktop-journey.ps1`,
  `tests/workbench-history-browser.mjs` (the preview fixtures lost `mode`, `observed` and
  `authoring_allowed`), and the Linux acceptance `governance.mjs` (the strict-coverage phase became
  `excluded-frame-content-is-left-out-and-reported`) and `managed/run.mjs`; every policy document
  they write is schema 4. `node tests/history-compatibility.mjs` with
  `GHOSTLIGHT_PREVIOUS_EXECUTABLE` set gives the published predecessor a schema-3 file and the build
  under test a schema-4 one. On a real home holding a schema-3 `policy/user-policy.json`, starting
  the service moves it and leaves `user-policy.json.schema3.bak`; `ghostlight policy migrate` on a
  schema-3 file with `mode: "observe"` prints the refusal and changes nothing. `node
  tests/workbench-surface.mjs` and `node tests/policy-grammar.mjs` were run and pass (neither starts
  a Ghostlight program).
- f04: `node tests/history-compatibility.mjs` (it starts the built service, so only the coordinator
  runs it) passes; `node tests/process-journey.mjs`, `tests/provenance-journey.mjs`,
  `tests/script-browser-journey.mjs`, `tests/local-resilience-journey.mjs` and
  `tests/cli-powershell-journey.mjs` pass (their fake browsers no longer send `masked_regions`).
  `node --check` passes on all five. `node tests/live-journey.mjs` (it starts the service and a live
  browser) and `node tests/workbench-history-browser.mjs` (it reads the preview fixture in
  `tests/workbench-preview-server.mjs`) pass.
- f05: `node tests/process-journey.mjs`, `tests/frame-browser-journey.mjs`, `tests/installed-journey.mjs` and the Linux `governance.mjs` pass with the one-and-a-half-second wait after each policy edit (`node --check` passes on all four). The Linux `managed/run.mjs` (poll_seconds 1, a local signed bundle republished between phases) still passes: the bundle change now reaches `snapshot()` on the next one-second tick. On a machine with an HTTPS managed bootstrap, a service start answers its first tool call from the verified cache without waiting for the endpoint, and the first poll happens on the refresher thread. A hand edit to a policy file shows in `policy_explain` within about two seconds; a workbench edit shows at once.
- f09: `cargo test -p ghostlight-relay` on a Linux test machine (the new mode test in `relay/src/log/tests.rs` is Unix-only), and `cargo test -p ghostlight -p ghostlight-bridge` there for the Unix-only mode tests in `audit.rs`, `install/swap.rs`, `install/migration.rs`, `governance/migrate.rs`, `install/record.rs`, `governance/managed/mod.rs`, `governance/tests.rs` and `bridge/src/diagnostics.rs`. On a Linux home after a first run, `ls -l` shows 0600 files and 0700 directories under `~/.ghostlight/state`, `policy`, `diagnostics` and the audit file. `cargo clippy -p ghostlight` on the Linux target was not run (the orchestrator needs system libraries); relay and bridge were.

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
- f03a: the tree differed from the spec in four places. (1) Four more variants had the removed
  state as their only producer and went with it: `Readiness::NeedsYou` with
  `ReadinessFacts.needs_attention`, `OperationPhase::Attention`, `NotificationKind::Attention`,
  and the `GovernanceDiagnostics` and workbench snapshot field `runtime_control_file_configured`
  (the UI never read it; the two workbench fixtures drop it). The UI's running-phase test drops
  `attention`. (2) `RUNTIME_END` is renumbered 2 in the in-memory store; nothing persists it.
  (3) `GHOSTLIGHT_RUNTIME_CONTROL_FILE` is also gone from the man page, the Linux installed
  journey's cleared-variable list and `tests/history-compatibility.mjs`. (4) The man page, the
  scripting guide and the SIEM guide lose exit code 3 and the `attention_required` signal; the
  exit codes 2, 4, 5 and 6 are asserted by number in `each_status_keeps_its_exit_code`. The
  audit lines written before this slice
  carried `reason: runtime_attention`; `ReasonCode::RuntimeHold` reads that code through a
  read-only serde alias and never writes it, and
  `audit_reads_lines_that_carry_the_removed_attention_state` runs such a line through
  `audit::read_line`.
- f03b: the tree differed from the spec in seven places. (1) `mode` also lived on each grant, so
  `grants[].mode` leaves with it and migrate applies the same rule to it. (2) Removing the user switch
  took `user_authoring_allowed`, `AuthoringError::{NotAllowed, OrganizationOnlySetting}`,
  `UserLayer.authoring_allowed` and `Inputs.authoring_allowed` with it, and the policy_explain facts lose
  `documents`, `layers[].mode` and `rules[].mode`. (3) Channel admission left a whole vocabulary behind
  (`ReasonCode::ChannelDenied`, `PolicyRule::Channel`, `BlockedReason::Channel`,
  `OpeningRefusal::ChannelNotAdmitted`); none was ever written to audit, so no read alias was kept.
  (4) `Manifest` and its blocks skip absent optional fields when serialized, so a migrated file has no
  `null`s; migrate writes the document through `Manifest`, which orders keys by the struct, not by the
  original file. (5) A `.schema3.bak` that already exists with different bytes stops the move
  (the file is left alone) instead of being overwritten. (6) Doctor and the workbench show the migrate
  sentence through the shared `Readiness::PolicyInvalid` detail and the Status Authority card, not through
  a new diagnostics field, which would have carried a path. (7) `examples/scripting-disabled.json` is
  deleted as well as `developer-observe.json`: the channel switch was its whole purpose. The locked
  fleet example keeps its name and loses the user switch. The journeys that exercised the removed
  options changed rather than gained replacements: `tests/cli-journey.mjs` drops the channel refusal,
  `tests/frame-browser-journey.mjs` runs the one remaining coverage behavior, and the Linux
  `governance.mjs` phase for `complete_page` now asserts the permitted-content result.
- f04: the tree differed from the spec in five places. (1) The `AuditRecord` tests moved with the
  record into `audit/record_tests.rs` (declared from `audit.rs`), together with the golden and
  reader tests; `governance/tests.rs` keeps only the policy attribution test and imports the record
  from `crate::audit`. (2) The golden comparison is the existing helper in
  `language/self_check/golden.rs`, which gained `check_golden_with(relative, actual, variable,
  subject)`; `check_golden` calls it. (3) `flow.json` is a pretty JSON array of two records (the
  child receipt, then the parent); the other two hold one record each. (4) `AuthoritySnapshot`
  gained `managed_sequence()` because `with_policy` now lives outside `governance`. (5) A line
  whose `step.parent` is `sequence` no longer reads as a receipt (it counts as an unreadable
  entry), and a `browser_landing` line reads as an ordinary history item; both were pre-2.0 only and
  the spec deletes the types that handled them. `DocumentObservation` stays strict: it is built in
  the adapter and never read from a line.
- f05: the tree differed from the spec in six places. (1) The sources lock is split so a workbench edit never waits behind a fetch: the `ManagedAuthority` sits in its own `remote` mutex, which only the refresher locks (across the fetch); `sources` keeps the local files and a `RemoteView` copy of the managed authority and is held only for file reads; the test accessor `hold_sources` holds `sources`. (2) The published state carries the views the workbench reads, built at publish time, so `diagnostics`, `policy_chip`, `effective_authority`, `browser_startup` and `candidate_snapshot` read it too and describe one instant. (3) A local managed bundle that was rejected is no longer retried on every call: `source_fingerprint` is `None` until the source is first looked at, and an unchanged source (even a rejected one) is not read again until its state changes. (4) The edge case the spec did not name: construction must not fetch, so `ManagedAuthority::load_offline` skips only the HTTPS fetch and leaves `next_attempt` unset, which makes the refresher's first tick the first poll. (5) Test 4's probe is behavioral: a replacement bundle with the old size and modified time stays unseen, one with a moved modified time is seen; no read counter exists. Tests that edit a policy file under a running facade call `refresh_tick()` (`TestPolicy::set` in `work/mod.rs` refreshes every facade it gave out), and `snapshot()` callers that mutate a copy now clone out of the `Arc`. (6) `ServiceHost::join` waits for a refresher mid-fetch, which the fetch timeouts bound at about fifteen seconds; a stop with no managed HTTPS source is immediate.
- f09: the tree differed from the spec in four places. (1) The spec's `create_private_append` beside `atomic_replace` became `ghostlight_relay::private`, because the relay, the bridge and the service all need the same two calls and relay depends on no service crate; the service imports it. (2) The two private copies of atomic write (managed policy, policy migration) are deleted rather than patched. (3) Windows keeps today's behavior in every helper. (4) The relay's service lease and runtime-file directories use the same helpers; `install/user_assets.rs`, `desktop_entry.rs`, `native_host.rs` and `command_path.rs` write outside the home and are unchanged. `migration.rs` does write into the home (its copies, records and audit archive), so it uses the same helpers; `private::restrict_file` tightens the archive after its rename. `policy migrate <file>` runs on a file the person names, which may be a shared organization file, so it uses `atomic_replace_keeping_access`: the migrated file keeps the old file's mode on Unix, and only its `.schema3.bak` backup is private. The `policy keygen` seed directory is chosen by the person and is unchanged; its seed files were already 0600. f13b re-checks every writer.
- f06d: the tree differed from the spec in one place: the environment module had no sentence-list test entry for `BACKGROUND_POSTURE`, only its own pinned-value test, so only that test went. `docs/tasks/reference-experience/PINS.md` still quotes the sentence as history and is untouched.
