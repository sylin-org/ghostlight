# Connectors lane ledger

Lane rules are in [BOOTSTRAP.md](BOOTSTRAP.md). Target: ADR-0197 Decisions 1-4, 10, 11, 13-16.

## RESUME HERE

Next slice: the remaining deferrable connectors slices, per the lane plan. No blockers.

## Landed

- connectors-0: ADR-0197 recorded; the ADR index status cells and ADR header notes mark what it
  supersedes or amends; this batch opened.

- connectors-1: `crates/relay` (package `ghostlight-relay` 3.0.0): framing, the fixed home and
  file names, the runtime file, the opening and its replies, failure templates. Fixtures under
  `tests/golden/relay3/`, bound by `crates/relay/tests/golden.rs`. Workspace `rust-version` is
  now 1.89. Gate: 35 unit tests and 9 fixture tests pass.

- connectors-2: relay mechanics in `crates/relay`: `spawn` (backoff, detached `serve` launch),
  `watchdog` (parent identity and polling), `log` (marker-gated diagnostics lines), `client`
  (connect and read the reply under a deadline), `server` (bounded opening reader, constant-time
  token compare, the lease). Gate: 65 unit tests and 9 fixture tests pass.
  The `serve` command starts in `home`, never the connector's own directory, so a running service
  does not hold the user's project folder open; a test checks it. The Windows creation flags are
  one named value, tested as the union of the three documented flags. The Linux stdio and process
  group are tested through the command's alternate Debug text. Not covered by a test: the null
  stdio of the Windows launch, which `std::process::Command` does not expose.

- connectors-3: `ghostlight-mcp-connector` 3.0.0 rebuilt on `ghostlight-relay` (`main.rs`,
  `forward.rs`, `hold.rs`); its MCP handling (`mcp_2025_11_25.rs`, `mcp_2026_07_28.rs`,
  `service_session.rs`) deleted for the service lane to port. It forwards lines unchanged, tracks
  request ids per direction (batches per element), answers `ping` itself, holds lines while the
  service is away (4 MiB, 30 s each), starts the service only at its own start and for a held
  request, presents the resume token on reconnect, and answers cut-off and expired requests from
  the template file. Gate: 16 connector tests, 65 relay unit tests and 9 fixture tests pass.
  Known interim break: the bridge test
  `both_connectors_recover_through_the_shared_lifecycle_seam` reads the deleted
  `service_session.rs`; it goes with the bridge in the switch.

- connectors-4: `ghostlight-browser-connector` 3.0.0 rebuilt on `ghostlight-relay` (`main.rs`,
  `port.rs`); it depends on the relay crate and `anyhow` only. It holds the extension's hello,
  drops later Chrome frames until a service accepts a `browser` stream, starts the service on
  the backoff schedule only before that first connection, then pumps raw frames both ways on two
  threads without parsing. When the stream ends it writes `backend_unavailable` once, drops
  Chrome's frames, never starts the service, and probes with a read-only control `readiness`
  opening on the backoff schedule; the first accepted probe ends the run so the extension
  reconnects with a fresh hello. Gate: 11 connector tests, 19 MCP connector tests, 65 relay unit
  tests and 9 fixture tests pass. The bridge test that reads `browser-connector/src/main.rs`
  stays broken until the bridge goes in the switch, like the MCP one.

- connectors-5: `scripts/dev-loop.ps1` builds the workspace into `.target-dev-loop` (passed to
  cargo with `--target-dir`, so the shell environment is untouched) and hands the result to
  `ghostlight deployment install --development`; `-Restore` runs the installed binary's
  `deployment install --restore`; `-Plan` adds `--plan` to either. Output is read with a bounded
  wait so a detached service holding the pipes cannot hang the script. The script was
  parse-checked and not run. It depends on `deployment install --development/--restore/--plan`
  from the switch deployment slice.

- connectors-6: the installed protocol-4 journey. `tests/lib/cdp-pipe.mjs` launches branded Chrome
  on a scratch profile with `--remote-debugging-pipe` and `--enable-unsafe-extension-debugging`
  (never `--load-extension`), speaks NUL-framed JSON on fds 3 and 4, loads the unpacked extension
  with `Extensions.loadUnpacked`, closes through `Browser.close`, and copies the user-level
  native-host manifest into the scratch profile on Linux and macOS. `tests/lib/cdp-pipe.test.mjs`
  covers framing and id matching with a fake child (16 tests, no Chrome).
  `tests/installed-journey.mjs` refuses to run with `GHOSTLIGHT_TEST_HOME` set or without the
  installed runtime file and MCP connector, never starts a service, asserts the extension id,
  drives the installed MCP connector, and runs the named checks of the validation plan against
  fixtures on two origins (a `127.0.0.1` one and a `localhost` one, so frames are cross-site).
  `--live` adds the nine public demo checks; `--hold` keeps the scratch browser for the demo
  scripts. It was parse-checked, its fixture pages were served and script-checked, and its relay
  control stream was exercised against a fake listener; it was not run against Chrome or the
  installed service. Two things in it are guesses to confirm on the first run: the fields of the
  `ghostlight status --json` output (it reads `running`), and the user policy file name and schema
  in the policy check (`~/.ghostlight/policy/user-policy.json`, schema 4). The `--live` checks
  for pages other than desk, the iframe pair, brief and foundry are generic because those pages
  are not in the repository.

- connectors-7: launchers follow the 2.0 install rules (ADR-0197 D13, D14, D16).
  `packaging/npm/bin/ghostlight.js` classifies the command first. Four kinds never install:
  `doctor`, `status`, `help` (also `--help`, `-h`) and `--version` (also `-V`), plus the help of
  `install` and `uninstall` (exactly one `--help` or `-h` after the command, as the service parser
  accepts). Every other command installs when the set is absent or older, including no arguments
  (the MCP connector), `call`, `policy`, `diagnostics` and unknown commands. This is deliberately
  D13 over the slice text, which listed only no arguments, `install`, `open` and `serve` as
  launching: a command that needs the service must find an installed, current one. A launch runs
  the installed set when `~/.ghostlight/bin/install.json` `service_version` is the same or newer
  than the package (or the `~/.ghostlight/development` marker exists), and installs when the
  executable it would run is missing; otherwise it downloads, verifies against `checksums.json`,
  runs `<staged>/ghostlight deployment install` (no directory argument) and then the installed
  binary. Installed binaries are no longer hashed per launch. Never-install commands run the
  installed service whatever its version, or a verified staged copy that installs nothing and is
  removed on exit. On Linux it checks for `libwebkit2gtk-4.1.so.0` (`ldconfig -p`, then known
  directories) and names the package. The launcher's home override is now `GHOSTLIGHT_TEST_HOME`
  (was `GHOSTLIGHT_HOME`), so it reads the same home the service uses. `scripts/get.ps1` and
  `scripts/get.sh` call `deployment install` without a directory. `packaging/mcpb/server/launch.js`
  runs `~/.ghostlight/bin/ghostlight-mcp-connector`, running the bundled
  `ghostlight deployment install` first when the install record is absent or older than the bundle
  version or that connector is missing; its README notes 2.0.0 is the channel's final release.
  Gate: 25 npm launcher tests and 9 MCPB launcher tests pass; both get scripts parse. Not run:
  any download or install. Follow-ups outside this lane: `tests/linux/acceptance/npm/native.mjs`
  still sets `GHOSTLIGHT_HOME`, and `tests/fixed-installation-journey.mjs` still passes a
  directory to `deployment install`; both are switch-time work.

- connectors-8: the NSIS, deb, Scoop, WinGet, portable and Homebrew channels are retired
  (ADR-0197 D16). Their release.yml jobs, scripts and manifests are deleted, along with
  `crates/orchestrator/tauri.release.conf.json` and the Debian-only `packaging/linux/changelog`
  and `packaging/linux/copyright`, which only that config referenced. Kept because install still
  embeds them: `packaging/linux/ghostlight.desktop.hbs`, `man/` and `completions/`. Left for
  switch-19: `crates/orchestrator/tauri.conf.json` and `packaging/windows/hooks.nsh` (the two
  allowed exceptions, in the orchestrator crate). Left for switch-20: documents that still
  describe the retired channels (CHANGELOG, STATUS, release notes, task ledgers).

- connectors-9: connector guards. `tests/guards/connectors.test.mjs` (node --test) regenerates
  `cargo tree -p <crate> -e normal --target <target> --prefix none --locked --offline` for both
  connectors on `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`, strips machine paths and
  versions of path crates, and diffs against `tests/golden/cargo-tree/`; it skips with the cargo
  message when offline resolution fails. It also diffs `crates/relay`,
  `crates/mcp-connector` and `crates/browser-connector` against the last published tag at the same relay revision.
  Set `GHOSTLIGHT_GUARD_UPDATE=1` to rewrite the goldens. Crate counts (unique lines): 17 on
  Windows and 16 on Linux for each connector; the Windows extra is `ghostlight-win-peer`. The
  extension entry arrives with switch-17.

## Owed

- `crates/relay/src/log.rs` gets a test that pins its line's key set (and that only Event-vocabulary
  names appear) before the bridge diagnostics sink and its schema pin are retired (ADR-0145 D4).

## Blockers

None.
