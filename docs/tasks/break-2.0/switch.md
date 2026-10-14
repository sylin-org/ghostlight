# Break 2.0: switch ledger

The switch runs after the three lanes, one slice at a time, and owns every file. Ground rules are
in [BOOTSTRAP.md](BOOTSTRAP.md). The target is
[ADR-0197](../../adr/0197-contained-seams-and-the-2-0-break.md); the wire is
[adapter-protocol-4](../../design/adapter-protocol-4.md).

Gate, with `CARGO_TARGET_DIR=.target-assessment`: `cargo fmt --all --check`, `cargo clippy
--workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `npm test
--prefix extension`, the protocol-4 and level-3 page asset tests, `npm test --prefix
packaging/npm`, `node --test packaging/mcpb/test/launcher.test.js`, `node --check` on changed
scripts, and `scripts/check-repository-integrity.ps1`. No journeys and no binaries are started.

## RESUME HERE

Next: switch-6, the one 2.0 break migration.

Later slices adopt `install::record::atomic_replace` for every installer writer, and gather the
entry rule's facts through `install::entry::canonical_path`.

## Landed

| Slice | What | Commit |
|---|---|---|
| switch-1, commit 1 | The service serves relay protocol 3 on one loopback listener (`intake/listener.rs`): the lease is the relay lease on `home/ghostlight-runtime.lock`; `publish_ready` writes `relay-templates.json` and then the relay-3 runtime file; openings are read under the 5 s deadline with bounded pre-auth slots, the token is compared in constant time, and refusals carry a code and a sentence from `language/intake.rs`. `mcp` streams feed `McpSession`, send a resume token after initialize and after every result, and a resume token restores the revision, client, channel and the tabs one `ListTabs` still finds. `browser` streams attach to the browser port. `control` answers readiness, `activate_workbench` and `quit`. The command line is an MCP client (`cli/client.rs`) on the `cli` channel; `status`, `doctor` and `open` use control streams, and `call` and `open` start `serve` when no service answers. `GHOSTLIGHT_TEST_HOME` replaced `GHOSTLIGHT_RUNTIME_FILE` in the service; service tests hold one shared test home. | see `git log` |
| switch-1, commit 2 | The typed service bridge is gone: `service.rs`, `client.rs`, `relay.rs` and `runtime.rs` left `crates/bridge`. The session vocabulary lives in `intake/session.rs` without the serde it no longer needs; only tool definitions, the intake channel and the session marker serialize. The bridge's demand-start and its lease went with the runtime file they read; the installation record sits in the relay home and probes the relay lease. The Linux D-Bus activation test that drove the old demand-start is removed. | see `git log` |
| switch-2, commit 1 | Protocol-4 goldens bind Rust and JavaScript. `level4/assets.rs` builds the page assets once from `assets/world.js` and the sources its include marker names, exactly as `bundle.js` does; the id is pinned to `assets.sha256`, the source stays under 512 KiB and `hello_accepted` under 768 KiB. `RelayBrowserPort::new` takes the assets and the service hands them over, so a protocol-4 hello is always served. `level4/golden.rs` checks requests, receipts, frames, events, tolerance, world messages, closed names, asset error codes against `world.js` and dispatch kinds against `content.js`. Drift fixed on the Rust side: `input` omits an absent precondition or guard, and key steps omit absent fields. | see `git log` |
| switch-2, commit 2 | The service speaks adapter protocol 4 only. Any first frame that is not a protocol-4 hello hears `adapter_incompatible` with the language sentence and closes. The work vocabulary moved to `browser/adapter/work.rs` (re-exported as `crate::browser::adapter::*`) and the document types to `browser/adapter/documents.rs`. Level 3 is gone: `chromium/level3`, the wire frames and capability table, the MAIN-world bundle `chromium/assets*` and its runner, `InstallPageRuntime` and the ready gate, `Cancel` work, the per-request probe and quarantine, diagnostics publication over the browser wire, `ChildTabOpened`, `DiagnosticsToggleRequested`, the `Level` enum, `Adapter` selection and `BrowserPlatform`. `Family` stays. `chromium/level4` is now `chromium/protocol4`; command chunking lives in its link with the wire's transfer bounds. `BrowserError::CapabilityVersion` became `Unsupported {primitive}`, which language renders as an extension update. `BrowserSummary` lost `platform` and `protocol` (no surface showed them). Contract tests run against `FakeExtension`. CI, AGENTS.md, CONTRIBUTING.md, DEV-LOOP.md and ARCHITECTURE.md name the protocol-4 asset tests. Two extension tests that paired the level-3 worker with the deleted level-3 `content.js` are gone. | see `git log` |
| switch-3 | The extension speaks protocol 4 only. `worker.js` is the manifest worker; `service-worker.js`, `lib/{engine,documents,frames,topology,form-diagnostics,screenshot,debugger}.js` and the tests that loaded them are gone. The manifest is 2.0.0 without the `scripting` permission (its store justification and the guard list follow). `lib/shared.js` keeps what the worker and the remaining libs use, and `linkState` gains the `service_outdated` state; `lib/state.js` drops the Attention control state and the topology and operation keys. The popup loses the diagnostics row and names the outdated service; the options card does the same. `extension/README.md` describes the protocol-4 worker, `package-extension.ps1` and CI list the new files, and `compatibility.json` gains the 2.0.0 adapter row (it covers service block 2.0, so `adapter-compatibility.ps1` stays red until the version stamp in switch-19). | see `git log` |
| switch-4 | The 2.0 install root as typed seams, nothing wired but one cleanup. `install/layout.rs` names every path under the home; `record.rs` holds `install.json`, `decide` (versions are compared before the image is looked at, so an older payload is refused even over a damaged image and damage only repairs the same version), `atomic_replace` and the semver order; `swap.rs` renames a running image aside under `.old-<n>` and `ServiceHost::start` removes what is left beside its own executable once it holds the lease; `marker.rs` reads and writes the development marker; `entry.rs` is the pure entry rule over canonical paths (`canonical_path` resolves links and the Windows verbatim prefix, and a target that names the running executable runs instead of forwarding). | see `git log` |
| switch-5, commit 1 | One fixed home. Every path derives from `relay::home::home()`: the audit log in `home/state` (unless `GHOSTLIGHT_AUDIT_FILE`), MCP client locations and the handoff marker in `home/state`, user policy and the managed cache in `home/policy`, diagnostics in `home/diagnostics` and `home/diagnostics.on`; registrations name `home/bin`. The installation record and selection, `LaunchMode::Deployment`, `deployment::select` and `owns_registration`, `verify_serving_executable`, the deploy lock and the bridge `lifecycle` and `parent_process` modules are gone. `deployment install` copies changed executables into `bin` under `home/install.lock` until switch-7 rewrites it. `doctor` reports the home, `install.json` and the development marker. | see `git log` |
| switch-5, commit 2 | The entry rule runs in `main` through `cli/start.rs` before dispatch: development forwarding (same arguments and inherited stdio, the child's exit code; `serve` detached and the parent exits 0), carrier install (`deployment install` logic into `bin`, then the installed service runs the command), refusal of `serve` from any other path; `doctor`, `status`, `help` and `--version` never install, and a command that does not parse never installs. A `Launcher` trait holds every process start, so the tests drive the real helper on temp homes. | see `git log` |

## Notes

- From switch-5 the journeys that set `GHOSTLIGHT_TEST_HOME` need `bin/ghostlight` in that home
  (or a development marker): a connector starts `bin/ghostlight serve`, and `serve` from any other
  path is refused. `tests/process-journey.mjs` still asserts the deleted installation selection.
  switch-18 rewrites the journeys.
- The Debian `/usr/bin` checks in `install/command_path.rs`, `desktop_entry.rs` and
  `user_assets.rs` can no longer match, since registrations name `home/bin`; the pristine pass
  removes them.
- `status --json` now reports `{version, relay, running}`; `running` means a readiness control
  opening was answered.
- A channel that policy does not admit is refused at the opening with the `unsupported` code and
  the policy sentence. The channel switches go away in switch-11.
- The extension on dev still speaks level 3 until switch-3, so a dev build of the service and
  extension does not connect between switch-2 and switch-3.
- `tests/keyboard-browser-journey.mjs` and `tests/stress-journey.mjs` still read the deleted
  level-3 page assets; switch-18 rewrites the journeys.
- `tests/delight-journey.mjs`, `tests/frame-browser-journey.mjs`, `tests/keyboard-browser-journey.mjs`
  and `tests/linux/acceptance/cold.mjs` still name `service-worker.js`; switch-18 rewrites them.
