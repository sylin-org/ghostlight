# Latest coordination request

- Updated: 2026-08-13
- From: windows-codex
- To: linux-codex
- Status: requested
- Host: `test-01`, local user `test`
- Branch: `dev`
- Required source head: `7022c6bd073e76761a6460d6554d5d1c15177817`
- Subject: native Linux Ghostlight 1.0 recovery, implementation, deployment, and visible acceptance.

## Objective

Bring the native Linux development lane back to at least the readiness and user-experience quality
proved by 0.8, on the current 1.0 architecture. Finish every source, process, installation,
diagnostic, npm-launcher, desktop, browser, recovery, and upgrade proof that `test-01` can honestly
perform. Treat failures as product findings: diagnose the root cause, make the smallest correct
implementation change with regression coverage, rerun the affected and full gates, and commit each
logical change separately.

This request is owner authorization to perform the bounded implementation, user-level deployment,
test-browser installation when needed, configuration, restart, and recovery work required on this
development machine. The owner also explicitly authorizes reading and updating the gitignored
`local/MACHINE-STATE.md` and `local/NOTES.md` for this task, subject to the repository's privacy
rules. It is not authorization to merge `main`, tag, sign, publish packages, mutate a store or
registry, create a GitHub release, or announce a release.

## Owner constraints

These constraints are acceptance requirements, not preferences:

- Zero user-experience regressions from 0.8. Preserve product identity, icon, motion, visible cues,
  ordinary-profile behavior, setup convenience, diagnostics, and safe recovery unless the owner
  explicitly changed a contract.
- The checksum-bound npm launcher is mandatory. Test the real local package as an MCP launcher and
  as the route to native CLI subcommands. Unit tests alone do not close this gate.
- Preserve all accumulated 0.8 tests, knowledge, publication lessons, documentation, and machine
  evidence. The clean-room rule forbids copying old implementation code; it does not authorize an
  evidence or resource wipe.
- Do not reimplement `demo` or `demo-brief`. Their current scripts are historical/product surfaces,
  not a request for new Rust commands.
- Do not add a release conductor, checklist bureaucracy, generic framework, new architectural
  layer, or speculative scaffolding. Add code only for a concrete defect or acceptance invariant.
- Preserve unknown and foreign configuration. Removal is allowed only where an existing contract
  positively identifies a Ghostlight-owned artifact and the active test calls for its retirement.
- Do not delete, overwrite, or flatten the versioned directories already under `~/.ghostlight`.
  Preserve them as upgrade evidence. Deploy 1.0 into a new versioned candidate directory.
- Do not read or expose credentials, cookies, browser page content, private screenshots, raw MCP
  payloads, or browser profiles. Record only content-free metadata and results. Never commit a
  password or token.
- Work in the ordinary visible graphical session and normal Chromium profile for live acceptance.
  Do not substitute Playwright, headless Chromium, an isolated profile, Xvfb, remote debugging,
  emulation, or tool acknowledgements without visible/page-observed outcomes.
- Leave unrelated work and untracked files untouched. Follow `AGENTS.md`, including ASCII-only
  writing and the applicable ADRs.

## Known machine and source state

Windows Codex established the following before this handoff. Recheck it natively rather than
assuming it is still true:

- `test-01` resolved to `192.168.1.109` and presented pinned ED25519 host fingerprint
  `SHA256:xw/JKOjleL/nhvx06mWtXQSlec9Ue/ydUHbmFHopdNw`. This is a new or rekeyed host relative to
  the old `test-host-01`; do not silently reuse the old fingerprint.
- The host was CachyOS Linux, x86_64, kernel `7.1.8-1-cachyos`, KDE on Wayland, with an active local
  graphical session.
- Rust 1.97 was the default, and rustup also had the repository toolchain
  `1.95.0-x86_64-unknown-linux-gnu`. Node was 22.22.1 and npm was 10.9.4.
- WebKitGTK 4.1, AppIndicator, pkg-config, make, and GCC were present. No supported Chromium binary
  was found at the time of inspection. Install a distro-supported Chromium build if it remains
  absent; record the package and version, not profile data.
- No 1.0 process or native registration was found. Existing versioned Ghostlight directories from
  0.5 and 0.6 were present under `~/.ghostlight`; preserve them.
- A byte-exact 681-file snapshot of source head `7022c6bd` was transferred to
  `/home/test/ghostlight-1.0-7022c6bd`. Its source archive was
  `/home/test/ghostlight-linux-7022c6bd.tar.gz`, 7,287,337 bytes, SHA-256
  `d4e2b11fda9e703af46e4faad1eaf1513fa1fdbe8eeb6cfd46dcea7049481bd7`. This snapshot has no
  authority over a newer `dev` checkout; it is retained evidence of the first Linux run.
- The first native run passed formatting and strict clippy, then stopped in the Rust suite at one
  platform-dependent test. The remaining gates did not run against that snapshot.

## First defect to implement

The reproducible failure is:

```text
install::migration::tests::only_the_old_service_command_shape_is_owned
crates/orchestrator/src/install/migration.rs:306
```

The tested command is a quoted Windows 0.8 supervisor command:

```text
"C:\Users\u\.ghostlight\bin\0.8.0\ghostlight.exe" service
```

`command_is_old_ghostlight_service` currently applies the compiling host's `Path::file_stem()`
rules. On Linux, backslashes are ordinary characters, so the Windows command is not recognized.
The same test passes on Windows. This is a production migration defect because ownership
recognition must be deterministic for stored legacy command shapes, independent of the host that
checks the fixture or imported configuration.

Before editing, follow the repository exploration workflow and read at least:

- `docs/adr/0115-packaged-native-host-lifecycle.md`;
- `crates/orchestrator/src/install/migration.rs`;
- the path ownership/comparison helpers in `crates/orchestrator/src/install/native_host.rs`; and
- the exact sibling executable naming in `crates/bridge/src/lifecycle.rs`.

Make the root fix inside `migration.rs`. Reuse the current quoted/unquoted command parser and exact
argument-shape check. Classify only a final component exactly equal, case-insensitively, to
`ghostlight` or `ghostlight.exe`, accepting both `/` and `\` as stored path separators. Do not add
a shell parser, dependency, type registry, cross-platform path abstraction, or new file. Preserve
the separate native `Path` use for current-platform launchd inspection.

Extend the focused tests so all hosts prove:

- a quoted Windows `ghostlight.exe service` command is owned;
- a Unix `ghostlight --instance qa service` command is owned;
- case-insensitive Windows executable spelling is owned;
- `ghostlight.cmd`, `ghostlight-old.exe`, a foreign executable, and a wrong argument shape are not
  owned; and
- the Linux unit definition still requires both the exact identity marker and owned command.

Run the focused test first, then the complete workspace. Commit this correction separately with a
conventional `fix(...)` commit before starting deployment work.

## Complete native source gate

Use rustup toolchain 1.95.0, the locked dependency graph, and one explicit fresh target directory.
Make `GHOSTLIGHT_BIN_DIR` point at that exact directory so process journeys cannot pass against stale
binaries. At minimum run:

```sh
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.95.0 build --workspace --locked --target-dir .target-linux-1.0
cargo +1.95.0 test --workspace --locked --no-fail-fast --target-dir .target-linux-1.0
npm test --prefix extension
npm test --prefix packaging/npm
node --test packaging/mcpb/test/launcher.test.js
sh -n scripts/get.sh
GHOSTLIGHT_BIN_DIR="$PWD/.target-linux-1.0/debug" node tests/process-journey.mjs
GHOSTLIGHT_BIN_DIR="$PWD/.target-linux-1.0/debug" node tests/cli-journey.mjs
node tests/workbench-surface.mjs
cargo +1.95.0 audit
cargo +1.95.0 deny check licenses bans sources
```

Also run `node --check` on every tracked JavaScript or module file and the repository's deterministic
public-surface, 0.8 recovery, artifact, and integrity checks if PowerShell is available. If
PowerShell is absent, either install a normal supported package when low-risk or record those exact
PowerShell-only checks as already passed on Windows and not rerun on this host. Do not substitute a
home-grown duplicate checker. Record exact test counts and every non-fatal dependency warning; do
not describe `cargo audit` as warning-free if it reports the known Tauri/GTK chain warnings.

## Native build and user-level deployment

Build the optimized Linux sibling set from the tested commit. Verify all three executables report
the same 1.0 version and place them in a new directory such as:

```text
/home/test/.ghostlight/bin/v1.0.0-dev-<short-head>
```

Do not mutate an older version directory. Hash the deployed binaries and compare them with the
tested build outputs. Use the existing install/native-host/doctor/status surfaces; do not create a
new deployment wrapper. Register supported browser native messaging and at least the Codex MCP
harness through Ghostlight's owned configuration seam. Verify unrelated JSON, JSONC, and TOML
content is unchanged.

Run the desktop authority in the active KDE Wayland user session, not as root and not under an SSH
session without the graphical environment. Prove:

- normal first launch creates the tray and starts with the workbench minimized;
- a second direct launch reveals and focuses the same workbench without a second authority;
- closing the workbench hides it while the authority continues;
- tray open and quit work;
- explicit `--headless` creates no desktop runtime;
- both connectors demand-start the trusted sibling authority after it stops;
- native registration points at the newly deployed browser connector;
- `doctor`, `doctor --fix`, `status`, and `status --json` are truthful and actionable; and
- no Run key equivalent, scheduled task, launchd agent, or systemd user supervisor is created.

Build the Linux release bundle and portable/raw forms that this host supports, inspect their exact
three-binary, UI, icon, license, and native-host payloads, and record hashes. `test-01` is CachyOS,
not Ubuntu. A source/user-candidate pass here does not satisfy Debian package-manager install,
signature, uninstall, or clean Ubuntu lifecycle gates. You may build and inspect a `.deb` if the
normal toolchain supports it, but do not convert it with an unofficial repackager and do not mark
`docs/testing/linux-live-lifecycle.md` L1-L9 as passed by a user-level Arch deployment.

## Mandatory npm launcher proof

Create a disposable local staging copy of `packaging/npm` bound to the exact optimized Linux raw
binaries and their real SHA-256 values. Use existing packaging scripts when available; do not edit
the checked-in placeholder manifest and do not invent a permanent helper. Pack it with `npm pack`,
install or invoke that tarball in a fresh temporary npm consumer, and preseed or serve only the
candidate bytes through the launcher's existing supported test seams.

Prove from the packed tarball, in real processes:

- the bare `ghostlight` command starts the MCP stdio connector, completes initialization, lists the
  catalog, and completes one safe browser call;
- `ghostlight install`, `doctor`, `status`, and one `call` subcommand reach the native orchestrator;
- the selected platform mapping is Linux x86_64 and all three cached binaries match the manifest;
- a tampered cache entry is rejected and replaced only by checksum-valid bytes;
- unverified or incomplete bytes never execute; and
- a second normal invocation reuses the valid cache without changing behavior.

Do not contact or publish to npm. Do not weaken the launcher to accommodate local testing. Record
the tarball hash, launcher version, cache location shape, and behavior only.

## Visible browser and MCP acceptance

Use a supported Chromium family in the ordinary visible `test` user profile and an explicitly
reloaded unpacked extension built byte-for-byte from the same tested source. Preserve the extension's
fixed identity and original assets. An unpacked extension is valid for this development proof but
does not satisfy the final matching-store-adapter release gate.

Use a fresh real Codex MCP session through the installed npm/native launcher. Run the current
acceptance matrix in `docs/1.0/ACCEPTANCE.md` and the applicable development portions of
`docs/testing/linux-live-lifecycle.md`. At minimum prove visible, page-observed outcomes for:

- first read and structured page inspection;
- ordinary navigation, tab creation/listing/focus, group reuse, child-tab adoption, and guarded
  local close;
- screenshot and post-capture presentation;
- semantic form input, ordinary typing, shortcut input, coordinate click, scroll destination,
  native and pointer-only drag, file upload, and dialog handling;
- JavaScript execution with its visible work cue and bounded result;
- governed denial that performs no browser effect;
- protected input whose cue and records reveal no secret value;
- concurrent sessions/workspaces from Codex and one additional available harness;
- orchestrator stop/demand-start without connector restart;
- browser restart and extension reload recovery;
- one interrupted effect reported as unknown and never replayed;
- visible border/cue/medallion behavior and content-free workbench history agreeing with the MCP
  result, browser receipt, and payload-free audit; and
- zero tabs inserted into an unrelated active browser window.

Check the full current user journey too: install, extension connection, harness connection,
successful first task, diagnostics/recovery, explicit integration removal, and retained-state
explanation. Do not reimplement or require the obsolete demo scripts as a gate.

If live use exposes a product defect, stop that journey, reproduce it narrowly, read the owning ADR
and closest current pattern, implement the root fix without fringe or extension policy leakage, add
regression coverage, commit it separately, rebuild/redeploy all affected processes, explicitly
reload the extension when it changed, and rerun both the affected journey and the full gate.

## Upgrade, recovery, and uninstall development proofs

Preserve the current machine state before each destructive lifecycle stage using content-free path,
ownership, version, and digest evidence. Do not save credential or page contents.

Where an official public Linux 0.8 artifact supports this host, install it through its documented
user path without deleting the existing browser profile, harness configuration, audit, extension
settings, or older version directories. Then upgrade to the 1.0 user candidate and prove:

- recognized owned 0.8 supervisor commands retire on first 1.0 launch;
- foreign or malformed lookalikes are preserved and named for attention;
- stale owned native and harness paths become explicitly updatable and then current;
- all three current sibling paths are exact;
- browser identity, unrelated harness content, audit/history, and extension settings survive; and
- affected visible journeys still pass.

If 0.8 has no compatible native artifact for CachyOS, use its documented portable/user install and
say so. Do not relabel a synthetic fixture as a package-manager upgrade.

Exercise recovery for stopped authority, missing owned native registration, disabled extension,
malformed owned harness configuration, expired managed authority, and unavailable native
notifications. Finish with an uninstall/reinstall cycle for the user candidate that proves only
Ghostlight-owned current registrations and selected harness entries are removed, retained
audit/history is explained, and older version evidence plus unrelated configuration remains.
Restore the tested 1.0 development candidate as the final active state unless the owner directs
otherwise.

Login/reboot work must use the actual local graphical session. Coordinate visible/reboot steps with
the owner rather than approximating them. After login and after reboot, prove there is still no
resident supervisor and that browser or MCP demand-start restores one authority, the tray,
workbench continuity, and a new successful call.

## Evidence and documentation

Keep only content-free evidence: UTC date, host/distro/kernel/desktop/display, architecture, browser,
Rust/Node/npm/Ghostlight versions, source commit, binary/package/tarball hashes, command exit status,
test counts, durations, process counts, registration path classes, and concise visible outcomes.
Scrub usernames from public documentation where the established template does not require them.

Update durable truth after the implementation and reruns:

- `docs/STATUS.md` with exact Linux source/native results and remaining release blockers;
- `docs/testing/release-readiness-2026-08-13.md` with the new tested implementation revision;
- `docs/testing/linux-live-lifecycle.md` with a clearly separated CachyOS development-host record,
  leaving Ubuntu/Debian/store/signature rows unpassed unless actually proved;
- `docs/MEMORY.md` only for a new durable cross-cutting lesson or owner preference;
- `local/MACHINE-STATE.md` and `local/NOTES.md` for machine-local paths, current active candidate,
  the new SSH identity, and sensitive working context, never credential values; and
- the applicable task ledger only if current work is already governed by one.

Run repository integrity, ASCII, and diff-hygiene checks after documentation edits. Every commit
must leave a green tree. Use one conventional commit per logical defect and one final docs/evidence
commit. Do not amend or squash already shared history.

## Completion and reply

When the native lane is genuinely complete:

1. Push the logical commits to `dev`; do not push another branch unless required to preserve
   unrelated concurrent work.
2. Replace this file with a concise latest result containing source head, active candidate path,
   exact gates/counts/hashes, npm proof, visible outcomes, defects and fix commits, host limitations,
   and every remaining release blocker.
3. Append the next numbered `linux-codex` reply to `coordination/CHAT.md` following
   `coordination/INSTRUCTIONS.md`, commit those coordination files separately, and push `dev`.
4. Do not merge `main`, tag, sign, publish, submit to a store or registry, create a release, or
   claim Ghostlight 1.0 release-ready. Those remain explicit owner decisions after all platform
   evidence exists.

If a required owner-visible action such as extension reload, ordinary-profile interaction,
logout, or reboot cannot be completed autonomously, finish every independent check first, record
the exact ready state without inflating it into a pass, and ask for that one bounded action.
