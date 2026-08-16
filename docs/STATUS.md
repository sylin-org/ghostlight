# STATUS -- Ghostlight 1.0 source candidate

Last updated: 2026-08-16.

This is the mutable implementation snapshot. Git history, the ADR index, dated research, and the
preserved `docs/0.8/` material carry history; this file does not rewrite it.

## Where the branches stand

Distances below are measured against the local remote-tracking refs, which are only as fresh as the
last fetch.

The repository carries exactly two branches, `main` and `dev`, as of 2026-08-13. The topology is
linear: `main` is an ancestor of `dev`, and nothing anywhere needs merging.

## Reference experience epic

The owner-approved product direction is captured as the staged
[reference-experience task batch](tasks/reference-experience/). It is READY at S1 and has not
changed production behavior.

The epic moves the center of delight away from setup surfaces and toward invisible ordinary use,
bounded automatic readiness recovery, one truthful pause/resume/stop contract, a calm At a glance
workbench front door, and a small optional branded doorway on controlled pages. It explicitly keeps
Ghostlight at the mechanism boundary: canonical browser operations, authority, lifecycle, and
effect truth belong here; inferred user-task meaning does not.

Execution must keep the DDD modular monolith, the minimum number of meaningful moving parts, and
complexity centralized at the seam where each fact becomes knowable. S1 is the next action: audit
the current journeys and ratify the experience contract and measures before implementation.

## RAWX and managed-policy restoration

The 1.0 policy product is restored on current orchestrator seams through `44f84eae`.

- RAWX is an independent set, not a rank. The exact action directory drives enforcement, audit,
  catalog projection, explanation, simulation, and tests. Sequence steps are admitted separately.
- Strict schema-3 ordered grants, host polarity and specificity, observe/enforce, layer
  intersection, sacred destinations, stable denial ids, grant attribution, and managed publish
  sequence are live.
- Policy-aware catalog projection emits the standard MCP tool-list change notification while
  all-open remains the exact 22-tool catalog.
- Local and managed policy reload atomically for future snapshots. Bad replacements keep last
  valid; configured cold start without authority fails closed.
- Customer-hosted file or HTTPS delivery uses signed monotonic bundles, required Ed25519, optional
  mandatory-both ML-DSA-65, verified cache, ETag, CA pin, bearer option, bounded retry, rollback
  refusal, and local signing/publication commands. No bootstrap performs no network work.
- The workbench Policy Passport shows organization, rationale, contacts, verification, sequence,
  freshness, source class, and verification time without source addresses, credentials, keys, or
  rules.
- The persistent workbench lamp band now distinguishes all-open, applied policy, retained-policy
  reload warning, and fail-closed policy states, with one click through to Status.
- Repeated enforced denials enter the existing workspace-local attention and runtime-control path.
  The full workspace gate passes with 197 orchestrator library tests, 2 orchestrator binary tests,
  30 bridge tests, 4 MCP connector tests, warnings denied, and 101 extension tests.

## The policy a person can read, and author

[ADR-0122](adr/0122-readable-policy-destination-and-authored-user-layer.md) is implemented, from
research input [24-policy-surface-user-delight-2026-08.md](research/24-policy-surface-user-delight-2026-08.md).

- Policy is the window's fourth destination. The state chip moved into the tab row between Status
  and About and opens it; Status keeps diagnostics, the session control, and notifications.
- One orchestrator-owned projection compiles the answer: a situation sentence, one line per
  capability in plain verbs with the layer that decided it, the rules behind those lines, the
  permanent ceilings in every situation including all-open, and the exact document and path for
  every layer. The surface renders it and computes no policy words of its own.
- Schema 3 gained an optional additive `organization` block (name, statement, HTTPS url, contacts).
  A signed bundle's presentation block still wins on conflict. Manifests stay typo-closed, so a
  document using the block is refused by older builds.
- The workbench authors one user policy through two bounded commands over a product-owned path in
  the per-user state directory. `GHOSTLIGHT_POLICY_FILE` still wins when set, and that file is
  shown read-only. Applying validates before replacing and writes atomically; no window action can
  leave the product failing closed.
- `policy.user.enabled` is registered. It gates authoring only, never enforcement, and is recorded
  as an operational control rather than a security boundary.
- Rules render as one list in evaluation order, organization first, each a single line that opens
  into detail: read-only for a rule this person cannot change, the editor for one they can.
- A capability line states polarity. Available, some sites blocked, some sites allowed, and not
  available are four distinct answers, and the middle two point opposite ways.
- The editor speaks sentences: host readback on every pattern, organization ceilings shown on the
  control itself, redundant and unreachable rules marked in place, watch-only as a plain switch,
  and a dry run against recorded audit before applying.
- The editor authors the registered settings, presented as three grouped permission toggles --
  where agents may connect, in the browser, privacy -- each on by default and named by what it
  does, never by its registered key. The permissive value is still never authored;
  `policy.user.enabled` is still refused from a user document; an organization ceiling on a setting
  disables its switch and names who set it, the same as a capability ceiling does. The two channel
  toggles link to the Integrations destination and the scripting guide instead of restating a
  client list that would drift.
- A refused row in the monitor names the deciding layer, the rule, the denial handle, and the
  organization's contacts when it supplied them.
- Gate at implementation: 212 orchestrator library tests, 2 binary, 30 bridge, 4 MCP connector,
  warnings denied, 101 extension tests, plus `node tests/policy-grammar.mjs` and
  `node tests/workbench-surface.mjs` (27 assertions). The amendment recorded in ADR-0122 covers the
  single rule list, stated polarity, authored restrictions, and the A4 permission-toggle framing.

The active 1.0 guides, contracts, public RAWX specification, pricing language, and Trust Center now
describe this feature set rather than the removed flat policy. The invented cohort-based
`greenfield-first-success.md` process is explicitly rejected as a release gate. Historical SPEC,
ADRs, research, and 0.8 task evidence remain preserved as history.

Direct UDP syslog from 0.8 was not rebuilt. The 1.0 audit contract is append-only local JSONL,
collected by the endpoint's existing file agent. Trust Center claims now say that plainly instead
of promising a sink that is absent. HTTP audit upload remains absent.

## Windows 1.0 development-host and package result

The Windows lane passed through implementation `b292bb22` on 2026-08-14. This is a
development-host package result, not clean-machine, provenance, login/reboot, matching-store-adapter,
or public-release evidence.

- `b979a8af` fixed the Windows first-run handoff state-root return found by the native compiler.
  `b292bb22` then fixed two packaged desktop regressions: release launches no longer expose a
  console, and the disposable workbench is created when Tauri's native event loop is ready instead
  of being lost just after startup.
- Formatting, warnings-denied workspace Clippy, all 194 Rust tests, all 100 extension tests, all 10
  npm launcher tests, all 4 MCPB tests, and the process, CLI, PowerShell, and workbench-surface
  journeys passed. The locked Windows build retained the Linux-only Tao patch from ADR-0120.
- GitHub CI run `31809913114` passed all nine jobs at pushed head `de4392db`: Windows and Linux
  Rust, Windows and Linux process journeys, both extension platforms, supply chain, release truth,
  and formatting.
- The mandatory npm process model preserved ordered CLI output and exit status from the optimized
  Windows application binary. A real installed MCP connector negotiated revision `2025-11-25`
  and returned Ghostlight's catalog metadata.
- The one-time npm handoff passed its first usable install, repeat-install, dry-run, `--no-open`,
  and CI-suppression cases. The same disposable install round registered Chrome, Edge, Brave, and
  Chromium plus Codex, Claude Code, Claude Desktop, Cursor, Visual Studio Code, Windsurf, Zed,
  OpenCode, and Crush with direct native connector paths. A second install changed zero bytes in
  the nine client configurations.
- The locked NSIS candidate is 3,292,239 bytes with SHA-256
  `100093627d781b1a4e0c8cc481d974e63fbce3939ad2383384c74f8915acb4d9`. Payload inspection found
  the exact three executables and four legal files. A silent install into an exact disposable
  directory ran its browser-registration hook, and doctor reported the full local chain current.
- Native HWND inspection found one visible minimized `Ghostlight` Tauri window and no visible
  console. A second launch restored and focused the same workbench, Close destroyed only that
  window while the authority remained alive, and a third launch rebuilt it. Exactly one authority
  process remained throughout.
- A first uninstall removed all four browser registrations and all nine owned MCP entries. A
  second uninstall changed zero config bytes and left no installed connector reference. The NSIS
  uninstaller removed every package-owned file and its uninstall record. Only test-created runtime
  and empty audit files remained; both exact disposable directories were then deleted. No
  Ghostlight process or default runtime file remained.
- The post-uninstall development checkout was restored on 2026-08-14 by rebuilding and deploying
  all three release binaries under `target/release`, registering the ownership-checked native host
  for Chrome, Edge, Brave, and Chromium, and starting the one local authority. The Foundry
  PowerShell runner now waits explicitly for the console-free Windows executable and validates
  stdout before reading result fields. Its real published-stage journey then completed every beat,
  including the off-domain refusal, replay delivery, and recording erasure.
- Chrome's extension-errors surface then exposed an unchecked `Native host has exited` report from
  a replaced port. The disconnect handler now consumes Chrome's callback-scoped `runtime.lastError`
  before its stale-port ownership guard, so an expected replacement cannot leave a false extension
  error while the current port is healthy. All 101 extension tests pass. The loaded unpacked
  extension still needs its one explicit reload and the historical error card cleared for final
  visible confirmation.

## Windows current-source and local-package pass

The [dated Windows record](testing/windows-current-source-pass-2026-08-15.md) covers revision
`72402a7d` without changing installed registrations or package state.

- Formatting, locked warnings-denied Clippy, a locked isolated build, all 288 Windows Rust tests,
  all 106 extension tests, all 10 npm tests, all 4 MCPB tests, every tracked JavaScript and shell
  syntax check, release truth, 0.8 recovery, repository integrity, dependency policy, and the
  advisory gate passed. The advisory result retains the same 17 documented allowed warnings.
- Fresh isolated process, CLI, PowerShell, workbench, and policy journeys passed against the exact
  build under test. The online public check found GitHub, npm, Chrome, the official MCP Registry,
  and the website in agreement about the public 0.8 state.
- An isolated optimized build produced the exact three siblings, a 4,528,043-byte unsigned NSIS
  package, a deterministic Windows portable archive, and a host-locally reproducible extension
  ZIP. Native package inspection found the complete sibling and legal payload.
- Exact HWND inspection proved minimized console-free startup, foreground activation, Close
  containment, and workbench recreation while one authority stayed alive. The exact test process
  and its runtime files were removed afterward.

This closes current-source, local-package construction, and non-installed native-window evidence
on Windows. It does not close clean install, public-0.8 upgrade, uninstall, login/reboot, tray,
notification, matching-store-adapter, public-harness, provenance-candidate installation, or
publication gates.

## Linux 1.0 development-host result

The native CachyOS lane was extended through the current 1.0 source candidate on 2026-08-14. This
is development-host and npm-candidate evidence, not a Debian package with verified provenance, store-adapter,
reboot, or public-release pass.

- A locally packed, checksum-bound `ghostlight@1.0.0` installed the current optimized Linux
  siblings into `~/.ghostlight/bin/v1.0.0`, updated the active Codex, Claude Code, Visual Studio
  Code, and four Chromium registrations, and handed the unpacked adapter from the older development
  candidate to that exact installed set. The orchestrator, MCP connector, and browser connector
  SHA-256 values are respectively `1cac38da4928dec72e8c6ceabdf92c4266142f76c1ba7bf2921a4cdb0a9e59ec`,
  `81058c3d41fb1815a46cca6e36bb01d5dc8fd864e457ed6f62b2e0a71bd14052`, and
  `a725e65a3a0ff9cfec760f064f876ebc28e1e946356b4a11875ef005095ed8b6`.
- A clean temporary npm consumer downloaded all three current Linux binaries through the launcher's
  injected release transport, printed one progress and one verification line per sibling, and
  rejected no expected checksum. A separately packed `npx --offline --package` install started
  from an empty home and config root, registered detected clients, opened the service-first
  walkthrough once, kept its marker mode `0600`, stayed non-interactive on reinstall, and reported
  the idle service as ready on demand.
- The active installed candidate passed `doctor` with its service and native adapter connected,
  then completed visible open, list, 19-word read, and 1248x615 screenshot against Example Domain.
  The default preserve-tabs interlock truthfully refused model-driven close. The service-first
  installer and extension-first adapter now lead to their opposite halves through the two stable
  pages; only a first real install opens a page, while CI, dry-run, `--no-open`, update, and repeat
  paths stay non-interactive.

- Rust 1.95.0 formatting, warnings-denied clippy, isolated build, all 191 workspace tests, all 99
  extension tests, all 10 npm tests, all 5 MCPB tests, 41 JavaScript syntax checks, both process
  journeys, the workbench surface journey, `cargo deny` bans/licenses/sources, and `cargo audit`
  passed. Audit still reports the 17 allowed GTK3/Tauri-chain warnings recorded in the dated
  readiness audit.
- The earlier three-sibling optimized user candidate at
  `~/.ghostlight/bin/v1.0.0-dev-6152636` remains as historical local evidence but is no longer the
  active browser stack. Its orchestrator, MCP connector, and browser connector
  SHA-256 values are respectively `97131236cdbb0be8367ce152182af8b8eaba8033f34c8d407a948ac5e20f58b3`,
  `73738e5d71ce6f20ad211c9b10082a5725ab32d05fe2fb9f447913c32662337d`, and
  `1631aed13e00aa0c22a8af8cdde259bcc56c1210dc88a4291441c8025c55cb50`.
- The installed workbench uses Tao's exact merged Wayland decoration fix. KDE reported a separate
  28-pixel server-side titlebar with closeable, minimizable, maximizable, and resizable state; the
  owner confirmed the controls accept pointer input. Native minimize/maximize/restore/close all
  landed while the authority stayed alive. The exact `Open Ghostlight` tray menu item rebuilt one
  active 1180x760 view after minimize and after close, and two concurrent Open events coalesced to
  one replacement without a duplicate label.
- A fresh offline consumer installed the packed npm launcher and proved bare MCP initialization,
  its 22-tool catalog, one safe browser call, native CLI routing, valid-cache reuse, tamper
  replacement, and refusal to execute incomplete or unverified bytes. No npm registry was
  contacted.
- Public attested 0.8.0 was installed through its documented portable path, then upgraded without
  removing the browser profile, settings, harness configuration, or any older version directory.
  The first run exposed a surviving Linux supervisor; the corrected migration now stops the
  positively identified unit before removing its unit and enablement. A final uninstall/reinstall
  removed only owned entries and preserved malformed or foreign configuration byte-for-byte.
- Ordinary-profile Chromium 151 under the active KDE Wayland session proved visible open, read,
  screenshot, presentation, single-authority activation, connector demand-start, and browser
  restart recovery. Local preserve-tabs correctly refused the attempted model-driven close.
  Closing/reopening the workbench, tray open/quit, login/reboot, a second live harness, and the full
  interactive form/drag/upload/dialog matrix remain owner-visible work.
- The portable archive was inspected and hashed. Tauri staged a complete AppDir, but its bundled
  `linuxdeploy` strip tool cannot parse CachyOS `.relr.dyn` sections, so no AppImage pass is
  claimed. The Ubuntu/Debian lifecycle table remains untouched and blocking.

## Lean Linux installation and visible activation

[ADR-0123](adr/0123-lean-linux-install-and-visible-activation.md) accepts and implements the
highest-value 1.0 findings from [Research 25](research/25-delightful-linux-experience-2026-08.md).

- `ghostlight open` composes the existing sibling demand-start and authenticated activation seams.
  Connector startup still passes no arguments, the authority still begins backgrounded, and no
  second service role, listener, wrapper, or resident supervisor was added.
- Linux user installation owns one XDG Applications entry and the existing 128-pixel icon. The
  entry names the exact installed executable plus `open`; updates rewrite only owned state,
  uninstall removes only owned bytes, and `/usr/bin/ghostlight` defers to the Debian package's
  system entry.
- Browser package provenance is a closed local fact separate from native-host registration.
  Default Linux setup selects detected native Chrome, Edge, Brave, or Chromium packages. Snap and
  Flatpak-only selections are refused with the native-package remedy; `--all-browsers` keeps the
  deliberate pre-registration route.
- The release Linux artifact builds on Ubuntu 22.04. Candidate assembly requires exact Debian
  package install/remove/reinstall/purge smokes in Debian 12 and Ubuntu 24.04. The local expanded
  matrix and GitHub candidate run `31920647296` both pass those gates. This does not replace the
  visible Ubuntu GNOME Wayland L1-L9 gate.
- Current-tree verification passed formatting, warnings-denied workspace Clippy, 274 Rust tests,
  103 extension tests, 10 npm tests, 4 MCPB tests, shell syntax, fresh isolated build, process, CLI,
  workbench, policy-grammar, Tauri-config build, dependency-policy, and advisory gates. The live CLI
  separately reported native browser provenance and missing Applications integration correctly.
  The rootless package lab below supersedes this source-only package limitation.

RPM remains the next rational native format only after a separate scope decision and a real
lifecycle host. AppImage, Snap, Flatpak, AUR, and Nix artifacts do not become 1.0 gates merely
because their packaging tools exist.

## Full local Linux release rehearsal

The current CachyOS user installation passed the fullest local release rehearsal at `51552025`.
The [dated record](testing/local-linux-release-rehearsal-2026-08-15.md) carries exact hashes,
commands, coverage, and limits.

- The supported uninstall removed only owned browser, client, and Applications state. The previous
  version directory was preserved on the second drive. Reinstall restored the exact three siblings,
  Chromium native messaging, Codex, Claude Code, Visual Studio Code, and the XDG entry. Repeat
  install changed nothing, and no product supervisor exists.
- Direct MCP negotiated `2025-11-25`, returned the exact 22-tool catalog, and completed a real
  call. CLI and PowerShell journeys passed. All 13 policy examples validated; explain, audit
  simulation, invalid-input refusal, Ed25519 authoring, live narrowed authority, fail-closed invalid
  authority, and all-open restoration passed.
- The published Foundry journey completed form, upload, recording, diagnostics, host refusal,
  replay, and byte erasure. Prompt handling and screenshot-bound coordinate input passed. Authority
  demand start, browser restart, stable browser identity, and post-restart work passed.
- The first Foundry run found that the clean-room drag path had retained only pointer packets and
  could wait forever for a held move receipt. `51552025` restores ADR-0088's bounded two-lane seam:
  explicit held-button packets, action-scoped native interception, opaque drag-data replay, and
  cleanup on every terminal path. The Foundry pointer lane and a native HTML drag/drop fixture both
  passed live after the fix. Extension coverage is now 106 tests.
- All 277 Rust, 106 extension, 10 npm, and 4 MCPB tests pass, as do formatting, strict Clippy,
  JavaScript and shell syntax, process/CLI/PowerShell/workbench/policy journeys, deterministic
  extension packaging, recovery and integrity checks, dependency policy, and the advisory gate.
- A fresh native Zed 1.15.0 installation exposed that CachyOS/Arch names its launcher `zeditor`.
  Zed's existing declarative harness descriptor now lists both `zed` and `zeditor`; the generic
  detector and single canonical config target remain unchanged. Ordinary unforced install created
  the exact `context_servers.ghostlight` entry, Zed showed it active, and the live process chain
  reached the exact installed connector. Repeat install changed zero config bytes. Removal stopped
  that connector while preserving other harness files byte-for-byte, and ordinary reinstall plus
  the documented client restart restored it. Zed's custom-server schema has no icon field, so its
  generated `G` and custom marker remain an observed host limitation. Ghostlight's own packaged
  visual identity and the roster expansion are now accepted by [ADR-0125](adr/0125-recognizable-plural-linux-harness-integrations.md).

This closes the broad development-host browser matrix previously listed as incomplete. It does not
close the provenance-bound Debian, matching-store-adapter, Ubuntu GNOME login/reboot, notification,
or three-public-harness gates.

## Recognizable plural Linux harness integrations

[ADR-0125](adr/0125-recognizable-plural-linux-harness-integrations.md) is implemented. The
[dated evidence](testing/linux-harness-roster-2026-08-15.md) records the complete development-host
roster pass and its release boundary.

- One fixed registry now has 18 products and 21 concrete targets. Added products are GitHub
  Copilot CLI, Cline, Kiro, Qwen Code, Junie, Kilo Code, goose, Continue, and Antigravity. Cline's
  CLI, Visual Studio Code, Cursor, and Windsurf targets remain independently owned below one card.
- Cards use packaged offline Ghostlight-owned visual marks and accessible product names. Missing
  products offer an official Install destination, Locate, Copy MCP command, and target-specific
  Copy setup. Detected targets offer Set up or Update; current targets offer Remove. Automatic
  setup failure opens the same manual route.
- Locate is one bounded native picker. Download URLs and clipboard material resolve in Rust from
  closed ids. The WebView has no generic dialog, clipboard, opener, shell, or filesystem grant.
- JSON, JSONC, TOML, and the new shared YAML seam preserve unrelated configuration. YAML tests cover
  comments, ordering, file mode, exact no-op bytes, owned removal, and refusal of flow shapes that
  cannot be edited losslessly.
- All nine new products were installed and started the exact isolated connector through their real
  Linux MCP lifecycle. Repeat setup and repeat removal were byte-identical across every target,
  and the final re-add restored all registrations.
- That live matrix found three compatibility blockers and closed each at its seam. Junie now
  negotiates `2025-03-26`; Antigravity receives `2026-07-28` discovery and falls back to an honestly
  advertised initialized revision; and Kiro/Bedrock receives portable top-level object schemas
  while the typed decoder retains exact conditional validation. No full stateless 2026 support is
  claimed.
- Current gates pass: formatting; warnings-denied workspace/all-target Clippy; all 294 Rust tests
  (252 orchestrator library, 4 orchestrator binary, 32 bridge, 6 MCP connector); 106 extension,
  10 npm, and 4 MCPB tests; every tracked JavaScript syntax check; process, native CLI, PowerShell,
  workbench (34 assertions), and policy journeys; repository integrity; dependency license, ban,
  and source policy; and `cargo audit` with the same 17 documented GTK/Tauri-chain warnings.

This closes current-source roster compatibility. The source-roster pass itself did not provide
package provenance; the build-only candidate below now does. Ubuntu GNOME Wayland,
matching-store-adapter, login/reboot, and publication remain open.

## Provenance-bound build-only candidate

The [dated candidate record](testing/release-candidate-2026-08-16.md) carries exact run links,
hashes, provenance checks, and remaining limits. Source revision
`fd8640336b11ed12cd47fe96deb7eb06adfbdcd1` passed ordinary CI run `31920645118` and manual
build-only candidate run `31920647296`.

- All nine cross-platform CI jobs passed, including Windows and Linux Rust and process journeys.
- The release quality gate, Ubuntu 22.04 Debian build, Windows 2025 NSIS build, deterministic
  extension build, Debian 12 and Ubuntu 24.04 package lifecycle smokes, and candidate assembly all
  passed.
- The candidate contains 17 checksum-bound artifacts and four CycloneDX SBOMs. All 17 hashes
  matched locally. GitHub provenance verified for every asset plus the manifest and checksum file,
  pinned to the exact repository, release workflow, source revision, and `dev` ref.
- The GitHub bundle has 14-day retention. No tag, release, submission, or publication was created.

This closes candidate construction, provenance, and the two accepted headless Debian package
gates. It does not close visible Ubuntu GNOME Wayland, matching-store-adapter, clean Windows,
login/reboot, notification, public-harness, or publication gates.

## Rootless Linux package evidence

[The dated container record](testing/linux-container-evidence-2026-08-15.md) carries the exact
candidate, image digests, coverage boundary, and results. The source candidate
`a9bd73424198cb144154117ad4dcae682d18baf5` produced a 4,768,536-byte Debian package with SHA-256
`a6c898f9072ae50363b12e8d422f74a6718d2bce3a874bd82d6d25b9658338e9` in a rootless Ubuntu 22.04
builder on the second drive.

- Debian 12, Debian 13, Ubuntu 24.04, and Ubuntu 26.04 passed the same package journey. It checks
  control metadata, dependencies, package checksums, modes, missing libraries, RPATH, the GLIBC
  ceiling, four conffile-bound native manifests, desktop validity, ordinary-UID runtime mode,
  status, doctor, native-host reporting, MCP initialize, remove, reinstall, purge, owned cleanup,
  and retained user state. Every binary's maximum required GLIBC symbol is 2.34.
- Ubuntu 24.04 passed that complete journey with its network namespace disconnected. The portable
  archive separately passed XDG install, exact idempotency, version-path update, runtime startup,
  and ownership-safe uninstall. Its SHA-256 is
  `7bf2994067c148191d797c572abd1a3604b487497c4bef8e2a44fb04548f8d10`.
- The attested public 0.8.0 archive installed its real user supervisor and browser manifests. The
  packaged 1.0 command retired the unit and enablement, rewrote all four owned manifests, changed
  zero bytes on repeat, preserved unrelated state and all old binaries, started as the ordinary
  user, and left user state after package purge.
- Advisory `lintian` now reports only browser-mandated `/etc/opt` paths, absent manpages for the
  three sibling executables, Rust-inapplicable C fortify notices, intentional duplicate legal
  resources, and binary string-table false positives. Placeholder metadata, libc dependency,
  conffile, changelog, copyright, strip, archive compression, and package-name path defects found
  by the first run are fixed and guarded.
- The release workflow and local guests now share one package lifecycle script. Extra local
  distributions remain advisory rather than expanding the accepted two-row release gate.

This local record supplied strong headless package evidence but not provenance. The build-only
candidate above now supplies matching GitHub provenance and the accepted two-row package smokes.
Ubuntu GNOME Wayland L1-L9, the matching store adapter, login/reboot, tray, notifications, and the
full visible browser matrix remain owed.

- `dev` is the working branch and the 1.0 source candidate. Workspace version `1.0.0`. It absorbed
  `ghostlight-1.0`, which was a fast-forward and has been retired.
- `main` carries the 0.8 line at `0116feca`. Promoting it is a deliberate release decision, not
  routine sync. The 1.0 line now carries Windows and Linux source, extension, process, and
  supply-chain CI; a manual Pages deployment; and bounded monthly dependency updates targeting
  `dev`. Manual build-only run `31920647296` created, inspected, and attested the current candidate
  without publishing it. Visible native operating-system validation and publication remain owed.
  Do not promote `dev` before those live gates pass.
- No pull requests are open. Thirteen Dependabot bumps against the 0.8 line were closed as obsolete
  on 2026-08-13: the 1.0 tree either already carried the proposed version or had dropped the
  package outright (`clap`, `rustls`, `webpki-roots`, `color_quant`). Dependency updates are paused
  on `main` with `open-pull-requests-limit: 0` rather than by deleting the configuration. The 1.0
  config targets `dev`, runs monthly, groups non-major updates, and caps open work per ecosystem.
- The pre-1.0 worktree snapshot is preserved as the annotated tag `archive/0.9-pre-1.0`
  (`f5d43768`), pushed to the remote. It replaced a local-only branch that existed on one machine.
  It is history, never implementation authority for the 1.0 tree.
- The 0.8 recovery is now source-backed rather than implicit. `docs/0.8/HARVEST.md` distinguishes
  the released, reconciled, mature, and archived snapshots; `docs/0.8/test-inventory.json` records
  1,354 ordinary test declarations and 34 source-enumerated Lightbox scenarios; and the dated
  publication observation corrects WinGet to merged while recording Glama drift. The old ledger's
  unexplained claim of 37 Lightbox scenarios is preserved as a discrepancy, not repeated as fact.
- Release safeguards are active again on `dev`: Rust, extension, and process CI cover Windows and
  Linux; dependency licenses, sources, wildcards, and
  advisories are gated; source and observed-public versions are checked separately; and the store
  extension package is built from an explicit runtime allowlist. The online public check passed
  against GitHub, npm, Chrome, the official MCP Registry, and sylin.org on 2026-08-13. The combined
  CI workflow passed all nine jobs at pushed Windows-lane head `de4392db` on 2026-08-14.
- Packaged native-host lifecycle is restored without restoring the 0.8 resident supervisors
  (ADR-0115). The orchestrator now checks, installs, updates, and safely removes Chrome, Edge,
  Brave, and Chromium registrations; packages carry both connector sidecars; and narrow migration
  retires recognized pre-1.0 Windows and Linux supervisor artifacts. The Windows NSIS
  candidate passed payload, install, doctor, idempotency, workbench-lifecycle, and uninstall checks
  on this development host. Clean-machine, provenance-verified, login/reboot, 0.8 package-upgrade, and Linux
  native-package journeys remain required evidence.
- The 0.8 test recovery is now dispositioned rather than merely counted.
  `docs/0.8/RECOVERY-MATRIX.md` maps all 1,388 entries through twelve current behavior areas;
  `docs/0.8/test-recovery.json` gives each of the 34 Lightbox process scenarios an explicit
  reexpressed, superseded, invariant-retained, or deferred state; and CI checks the map against the
  source-derived inventory. Two missing high-value proofs were added: sibling runtime discovery
  does not depend on Linux session environment, and an unreachable configured managed authority
  fails closed from cold start.
- The file-level harvest now names and content-addresses 809 in-scope artifacts from the mature 0.8 tree.
  Four high-value absences are restored on current seams: static Windows runtime policy, a narrow
  live-swap command, Chrome OAuth recovery, and independent Chrome publication. The checked ledger
  still detects a removed, newly restored, or identical-to-evolved path, but an already-evolved
  active file may keep evolving without an 809-row bookkeeping rewrite.
- Release construction is one checked 17-artifact unit: two native packages, two portable
  archives, six raw binaries, the deterministic extension, four component SBOMs, the npm launcher,
  and the Claude Desktop MCPB. Exact byte length and SHA-256 bind every item to one version and full
  source revision. The workflow adds GitHub build provenance but remains build-only. GitHub, npm,
  Chrome, and MCP Registry adapters each default to a non-mutating plan and require an explicit
  named action plus owner-approved execution; there is no master conductor.
- The 0.8 user entry points are restored on current seams. `npx -y ghostlight install` remains the
  primary journey; a bare npm launch remains MCP stdio; the launcher verifies all three cached
  binaries on every run. `ghostlight install`, `uninstall`, `doctor`, `doctor --fix`, `status
  --json`, `service`, dry-run, repeated client selection, and repeated browser selection are live.
  One-line installers, deterministic portable archives, the self-contained MCPB, and
  candidate-derived Scoop and WinGet 1.12 metadata are present and tested.
- Release access was recovered without exposing values. GitHub and npm authentication work, and
  the MCP DNS key and official publisher binary are present. Chrome API automation is incomplete,
  but 0.8 treated it as optional and used the Developer Dashboard when necessary. Ghostlight has no
  Windows code-signing certificate; 0.8 used checksums plus keyless GitHub provenance, and 1.0 now
  retains that model instead of inventing a signing gate.

## Implemented

- One Rust 2021 workspace builds four roles: the shared typed bridge, `ghostlight` orchestrator,
  generic MCP connector, and opaque browser connector.
- The orchestrator owns the 22-tool model-facing catalog, workspace aggregate, one executor and
  completion path, immutable authority snapshots, runtime controls, content-minimized audit,
  browser port, and content-free presentation decisions.
- The page-context JavaScript tool is `browser_execute`, not `browser_evaluate`. The execute name
  states that it may read, mutate, or navigate. The unreleased old name has no alias. Internal
  `RunScript` and `EvaluateScript` mechanism names remain behind the language boundary.
- The stable browser fringe includes a policy-free Manifest V3 extension, durable native relay,
  operation-disposition recovery, one browser-wide exact-title group per client label, dedicated
  Ghostlight window placement, and the established visual language and product identity.
- Adapter 1.0.0 advertises end-to-end liveness (ADR-0113). The service sends a content-free
  heartbeat every 20 seconds and follows every physical dispatch with its own probe. Forty-five
  seconds without an acknowledgement makes an attached relay unavailable; an operation deadline
  with no post-dispatch acknowledgement quarantines it immediately, so the next call stops before
  dispatch. A healthy silent operation stays connected when the extension answers independently.
  Older adapters retain their capability-gated attachment behavior, and the opaque browser
  connector is unchanged.
- Browsers are plural (ADR-0114). The service keeps one adapter connection per persistent browser
  identity, so Chrome and Edge, or two profiles, are connected and worked in at once. A hello
  carrying an identity that is already registered replaces that entry and **closes the replaced
  stream**, which is what makes a duplicate connection collapse instead of lingering as a silent
  sink. Each workspace binds to one browser for its life; physical tab ids resolve as
  `(browser, physical_id)`, so one browser's tab 5 can never be governed as another's. A crossing
  with no binding uses an explicit `browser`, then reported attention, then the sole connected
  browser, and otherwise refuses while naming the candidates. Runtime control publishes to every
  connected browser.
- Extension native-host startup is single-flight. Concurrent bootstrap, installation, startup,
  and reconnect signals share one attempt, and ownership is rechecked after local-state
  initialization. One worker epoch therefore cannot strand multiple attached relays with only one
  active extension listener.
- Recording now has one owner (ADR-0108, extended by ADR-0109). The extension keeps a plural,
  workspace-namespaced, memory-only registry; owns capture ids, frames, fixed bounds, autonomous
  stop, five-minute retention, erase, and the GIF encode itself; and exposes only
  start/status/stop/export/discard physical requests. It folds byte-identical successive JPEGs into
  one retained visual span with an accumulated duration, so capture time and compressed bytes are
  the ordinary limits. During recording, presentation disables only the perpetual controlled-scope
  glow and keeps transient action feedback available. The old service coordinator, renewal loop,
  unsolicited frame events, and duplicate deadlines are gone.
- Recording frames no longer cross a process boundary (ADR-0109). `gif_output.rs`, the
  frame-returning `read` command, and `PhysicalRecordingFrame` are deleted. The orchestrator
  governs the save, names one of three destinations, and states an output budget; the extension
  encodes in an offscreen document (pinned MIT `gifenc` under `extension/vendor/`) and delivers.
  A page attach and a browser download finish inside Chromium; only a client return carries bytes,
  and the shape of `RecordingDelivery` is what makes that structural rather than a rule. Thinning
  exists once, in `extension/lib/recording.js`, and folds each dropped frame's time into the frame
  before it, so a thinned replay still plays for as long as the work took. A saved replay's
  sentence reports how long it plays; counts and bytes stay in the facts. The manifest gained
  `offscreen` and `downloads`, which is a published-surface change.
- Model-driven tab close is admitted by service authority and then checked by the extension's
  default-on preserve-tabs interlock. A refusal stays visible and returns a blocked no-effect
  result.
- The `ghostlight` executable hosts a Tauri 2 workbench inside the modular monolith, with a tray
  lifecycle, bounded global search, and content-free native notifications. It presents three
  destinations:
  - **Monitor**, the landing surface. The current action stands in full with its elapsed time,
    then settles and drops into a newest-first queue as the next one rises. Connected sessions and
    browser instances sit alongside it, and the last completed action stays on screen while
    nothing is running.
  - **MCP integrations**, which checks, connects, and disconnects Ghostlight's owned registration.
  - **Status**, which carries diagnostics, authority sources, and the end-session intent.

  Pause and resume live in the persistent header beside the connection state and match the tray.
- The workbench capability grants the notification plugin only its automatic permission-state
  bootstrap probe. Notification delivery remains a Rust-owned presentation port; the WebView has
  no permission to request, send, cancel, or otherwise manage native notifications.
- The orchestrator publishes a closed sequenced change vocabulary (`OperationStarted`,
  `OperationChanged`, `OperationSettled`, `RuntimeChanged`) through a best-effort
  `WorkbenchEventSink`. Snapshots carry the sequence they reflect; a surface that receives a gap
  resynchronizes from a fresh snapshot rather than trusting its cache. The WebView may listen and
  is not granted permission to emit. A projection with no sink attached publishes nothing, so
  headless runs and domain tests stay free of presentation.
- `OperationSummary` carries the governed capability, so live work is classified as plainly as
  completed history.
- Monitor has a presentation-only Clear view control. It hides completed actions for the current
  desktop surface, keeps running work visible, and never mutates or deletes the durable audit.
- The workbench follows the published sylin.org palette: Ghostlight's teal accent carried as
  `--a`/`--al`/`--argb`, the night-garden ground, and the five-step ink ramp. The in-page renderer
  deliberately keeps its trained sky signal. The two surfaces still share the spring curve and the
  ADR-0083 medallion vocabulary.
- Supported MCP client registrations cover 18 products and 21 concrete targets: Codex, Claude
  Code, Claude Desktop, Cursor, Visual Studio Code, Windsurf, Zed, OpenCode, Crush, GitHub Copilot
  CLI, four Cline targets, Kiro, Qwen Code, Junie, Kilo Code, goose, Continue, and Antigravity.
  Re-check is read-only. Set up, Update, and Remove are explicit, serialized, ownership-checked,
  backed up, and preserve unrelated JSONC, TOML, and YAML configuration. Harness paths follow the
  effective Windows or Linux environment, including `CODEX_HOME`; exact owned pre-1.0 agent relays
  are migrated while other relay entries remain untouched and visible as attention-required state.
- `ghostlight --headless` retains the service-only execution path. Recoverable desktop startup and
  event-loop failures leave that service running.
- The shared bridge owns one demand-start seam used by both connectors after a failed service
  connection. It starts only the exact sibling `ghostlight` with no application arguments, honors
  a fresh deploy lock, and preserves each connector's established reconnect behavior.
- The orchestrator holds an operating-system lifetime lease before publishing runtime discovery or
  initializing Tauri. Concurrent launch attempts therefore converge on one authority and one tray.
- There is one normal desktop launch. It always creates the tray and backgrounds its workbench:
  minimized on Windows and hidden on Linux. A second direct launch opens and focuses the running
  authority's authenticated workbench. `--headless` remains the explicit presentation-free mode.
  Windows restores its existing view. Linux reconstructs its disposable view because Wayland
  cannot report or unset minimization.
- The tray and authority outlive their disposable workbench. Native close destroys the window,
  native minimize remains compositor-owned, and Open uses one serialized lifecycle seam. Linux
  coalesces Open requests, destroys any existing view, and reconstructs only after Tauri reports
  that exact window destroyed. Windows focuses or restores its existing view and constructs one
  when absent. Linux observes abnormal WebKit renderer loss, discards only that exact window after
  the callback, and recreates on the next explicit Open. The proprietary NVIDIA renderer policy is
  selected before WebKit starts and preserves user override.
- `ghostlight call` is a second intake for scripts and programs (ADR-0105). It invokes one tool, or
  a batch of them over one session with `--stdin`, prints the outcome sentence or `--json`, and maps
  the terminal status to distinct exit codes where an uncertain effect is never zero. It demand-
  starts the authority like any connector, and it crosses the same executor, governance facade, and
  completion path, so there is no scripting bypass.
- Every session records the intake it arrived on, and every audit record carries it. `ghostlight
  call` work is attributed to the `cli` channel and grouped under its own browser tab-group name.
  The channel is attribution and is never an input to an authority decision.
- `ghostlight_bridge::client::ServiceClient` is the one place the service handshake lives, so a
  second edge does not grow a second copy of it.
- `--output <file>` writes bounded content, so a scripted capture lands as an image rather than as
  base64 in a terminal. Later captures in one session gain an index instead of overwriting.
- A policy layer may close an intake: `{"channels":{"cli":{}}}` refuses it, `{"enabled":true}`
  admits it, and an absent map restricts nothing, so all-open is untouched. Layers intersect, so a
  managed refusal cannot be undone locally, and an unknown channel name is a typo that fails closed.
  The refusal lands at admission with the stable `channel_denied` reason, before a workspace exists,
  so nothing is invoked and nothing is audited (ADR-0105 amendment).
- A command-line session is its caller, not its connection (ADR-0106). Every `ghostlight call` from
  one terminal, or from one program that shells out repeatedly, reaches the same workspace and the
  same tabs. Identity is the caller's process id plus start time, so a recycled pid running the same
  program does not inherit a dead session; the executable name rides along for attribution only.
  `GHOSTLIGHT_SESSION` pins a session explicitly for a caller whose own children are ephemeral, and
  is a claim rather than an observation, so it never reaches an authority decision.
- An owned workspace outlives its connection and is released when its owner is gone, handing back
  the tabs it held. The close it then asks for goes through the same interlock a model's close does,
  so with the default-on preserve-tabs setting those tabs are released but stay visible. Liveness is observed rather than guessed at, sweeping on admission so the cost follows
  use. Work in flight is never reaped, and a connection that sends no marker keeps the previous
  connection-bound behavior, which is what the MCP edge does.
- [`scripts/demo-brief.ps1`](../scripts/demo-brief.ps1) and
  [`scripts/demo-brief.sh`](../scripts/demo-brief.sh) drive the ADR-0069-era launch-brief demo story
  entirely through the command line: open, scan, inventory controls once, three separately paced
  field writes, two checkbox clicks, submit, and a wait for the exact completion sentence.
  Verified live against the published Sylin stage: ten steps, one session, read/write/action
  capabilities classified per tool, and no typed value in the audit. `docs/design/demo-brief.md`
  specified this as a Rust subcommand; it does not need to be one, and the note now says so.
- [`scripts/browser-journey.ps1`](../scripts/browser-journey.ps1) and
  [`scripts/browser-journey.sh`](../scripts/browser-journey.sh) are complete PowerShell and POSIX
  shell journeys over the CLI: open, list, read, capture to a file, close, with Ghostlight's own
  exit code preserved. Each call remains a direct child of its long-lived shell, so each step uses
  the handle the previous one returned. [`scripts/demo-foundry.sh`](../scripts/demo-foundry.sh)
  gives the full Card Foundry story the same Linux-native entry point. All three shell scripts are
  syntax-gated and were verified against visible Chromium.
- Monitor rows carry the intake between the tool and the description, resolved from the record when
  settled and from the still-connected session while running. A guard derives the row's grid track
  count and each width's hidden cells from the stylesheet and compares them to the cells the surface
  renders, so a new column cannot silently shift the ones after it.

## Verified in this workspace

Re-run through 2026-08-15 against the current tree:

The complete dated evidence and explicit NOT RUN release gates are in
[`testing/release-readiness-2026-08-13.md`](testing/release-readiness-2026-08-13.md). The result is a
source-gate pass, not release approval.

- The follow-up gate repair made the managed-policy environment import Windows-only, moved the CLI
  refusal journey onto the maintained schema-3 policy example, scoped the reviewed
  `CDLA-Permissive-2.0` data-license exception to `webpki-roots`, and removed the unmaintained
  direct `rustls-pemfile` dependency in favor of the parser already exposed by `rustls`.
- `cargo fmt --all -- --check` and locked workspace/all-target warnings-denied Clippy passed.
- `cargo test --workspace --locked --no-fail-fast`: 264 Rust tests -- 227 in the orchestrator
  library, 2 in its launch-mode binary, 31 in the shared bridge, and 4 in the MCP connector.
- `npm test --prefix extension`: 103 extension tests.
- `npm test --prefix packaging/npm`: 10 launcher tests. The MCPB launcher has 4 Node tests.
- Fresh isolated debug binaries passed the Linux process and CLI journeys, including the schema-3
  CLI channel refusal. The workbench surface passed all 30 assertions, all 42 tracked JavaScript
  and module files parsed, and a fresh locked optimized workspace build completed. Dependency
  license/source/bans checks passed, and `cargo audit` returned to the documented 17 allowed
  Tauri/GTK-chain warnings after the direct PEM dependency was removed.
- The PowerShell-specific CLI journey was not rerun because PowerShell is absent on this host. The
  CachyOS host also cannot satisfy the exact pinned Ubuntu/Debian candidate and package inspection
  gate. The existing Windows and older development-host evidence remains unchanged, and the Debian
  L1-L9 lifecycle remains NOT RUN.
- The earlier cross-platform gate passed all four executable process/workbench journeys and all 41
  JavaScript and module files then tracked. This Linux follow-up reran the three journeys available
  without PowerShell and all 42 files now tracked. The 1,388-entry recovery matrix passed, every
  tracked file was readable with all local documentation links valid, and the artifact guard covers
  809 in-scope mature 0.8 paths.
- An isolated Linux command journey seeded an exact pre-1.0 Codex relay under a non-default
  `CODEX_HOME`, ran the built `ghostlight install --client codex`, and proved the Codex binary read
  the replacement MCP connector with empty arguments. The fixture used separate home and XDG roots
  and did not touch the active user configuration.
- The optimized local candidate then migrated the active Codex and Visual Studio Code legacy
  relays to the sibling MCP connector, reported both installed through doctor, and proved a second
  install was idempotent. The installed MCP edge negotiated revision `2025-11-25` with all 22
  tools. A fresh visible Chromium profile completed open and read against Example Domain; close
  stopped truthfully at the default-on preserve-tabs interlock.
- The restored development loop found the running repository service only by its exact
  `target/release/ghostlight.exe` path in plan mode. Its isolated smoke then built one selected
  package, enclosed the swap in `deploy.lock`, copied into a disposable repository-local live
  directory, removed the lock, and performed no launch under `-NoStart`. The real stack was not
  disturbed.
- Four real CycloneDX component SBOMs were generated with pinned `cargo-cyclonedx` 0.5.9. Synthetic
  Windows and Linux input exercised the exact 17-artifact assembly, npm hash embedding, deterministic
  MCPB, portable packaging, and package-manager metadata paths. That proves the construction code,
  not the missing native Linux artifacts or their provenance.
- Chrome, GitHub, and MCP publication plans made no mutation. That rehearsal exposed two 1.0
  release-adapter regressions from 0.8: optional Chrome API automation was described as required,
  and GitHub refused a candidate for lacking nonexistent platform signing. Both gates were removed;
  checksums and GitHub provenance remain mandatory. MCP Registry planning found the recovered key
  and publisher but refused the current 0.8 `server.json` and npm coordinate for a 1.0 candidate.
- The deterministic 1.0 extension ZIP has SHA-256
  `47a7cb7b715d14de991266f3602ecf6f166fd967623c4e7980f58a2afc3c47c3` and contains the exact
  Apache and MIT texts. The rebuilt local unsigned Windows NSIS candidate contains the exact three
  sibling executables and all four exact legal files; its SHA-256 is
  `100093627d781b1a4e0c8cc481d974e63fbce3939ad2383384c74f8915acb4d9`. Neither artifact is
  published or release-approved.
- `cargo audit` exited zero but reported 17 residual transitive warnings in the Linux Tauri/GTK3
  and Tauri URL-pattern graphs, including the `glib` iterator unsoundness advisory. The dated audit
  records the dependency paths and requires a recheck before Linux publication; do not call this scan
  warning-free.
- Plurality contracts prove two browser identities stay connected at once and each answers its own
  request, a second connection from one identity collapses onto the first with the replaced stream
  reaching end-of-stream rather than hanging open, attention moves to front without duplicates and
  never routes to an absent browser, and resolution prefers selection, then binding, then attention,
  then a sole browser. Executor contracts prove work follows the attended browser once and then
  stays there when attention moves, an ambiguous bootstrap names both candidates with no dispatch
  and no binding, a named stranger is refused rather than substituted, and listing tabs answers
  truthfully with no browser connected at all.
- Browser-port contracts prove an attached socket without heartbeat acknowledgements becomes
  unavailable, an unanswered post-dispatch probe quarantines at the operation deadline, a legacy
  adapter keeps compatible attachment semantics, and a silent operation can outlast the liveness
  timeout while independent acknowledgements keep it available.
- Lifecycle tests prove demand-start supplies no application arguments and the executable has one
  normal desktop mode beside explicit headless and scripted intake. The real process journey still
  passes across service restart and connector renegotiation.
- Action-subject tests prove the Chrome receipt carries the physical role and name without a
  describe round trip, the role cannot author language, editable values cannot become names, names
  are normalized and bounded, and either authority layer can remove them monotonically.
- `node tests/cli-powershell-journey.mjs`: the shipped PowerShell script drives a real service and a
  scripted browser adapter through open/list/read/capture/close, exits zero, writes real JPEG bytes,
  and every step is audited as `cli` with the landing host and no page text. It then proves the
  session marker across processes: one `ghostlight call` opens a tab and a separate one lists it.
- `node tests/cli-journey.mjs`: the real executable's command line reaches a real service, returns a
  governed result, exits non-zero on refusal, is attributed to the `cli` channel in the audit file
  the service wrote, and keeps one workspace across a `--stdin` batch while separate processes get
  separate workspaces. A second service started with `{"channels":{"cli":{}}}` refuses the intake
  with `channel_denied`, exits non-zero, and writes no audit record.
- `node tests/process-journey.mjs`: stable MCP and browser relays reconnect through a service
  restart without replaying an interrupted effect, then complete open/read, an extension-owned
  recording start/save/discard with a real GIF content block, a second save to the browser's
  download mechanism that returns no bytes at all, and close. Its adapter advertises liveness and
  acknowledges every dispatch probe through the unchanged opaque browser connector. The journey uses
  a fresh deployment lock to isolate explicit restart recovery from demand-start. It also reads
  the audit file the real executable wrote and checks that the read records a host and a word
  count, and no page text.
- `cargo build --workspace --target-dir .target-ghostlight-1.0`.
- `node --check` on both journeys, the bundled workbench script, and the preview server.
- A live 35-second static Example Domain recording with the scope glow suppressed retained 15
  frames and 121,293 JPEG bytes. Its 211,458-byte GIF was valid GIF89a, carried 35,320 milliseconds
  of playback with a 33,720-millisecond folded static span, and repeated save was byte-identical.
- A live Foundry hover, click, and type sequence retained six distinct frames across 670
  milliseconds. Its 595,861-byte GIF saved twice with the same SHA-256 digest, after which the demo
  state and recording bytes were cleared.
- Live isolated demand-start proofs began with no service. The MCP connector started one exact
  sibling authority and completed MCP initialization. In a separate run, the browser connector
  reported `backend_unavailable`, started one exact sibling authority, and completed its adapter
  hello. Each run found exactly one service at the isolated executable path and removed only that
  test-owned process afterward.
- The repository's live Windows `target/release` stack was replaced from an isolated release
  build under the deploy lock. Stopping its one service authority caused the already-running
  browser connector to demand-start one replacement with a fresh runtime token. A direct launch
  then revealed that workbench and exited while the authority count stayed one.
- The workbench renders against the repository preview server, which now drives the real sequenced
  event path, and uses the byte-identical original Ghostlight artwork.
- Guard tests keep the surface and the orchestrator in step: every publishable change has a
  handler, every capability class has a visual treatment, every runtime intent stays reachable
  (guarded by an exhaustive match), the surface reads the one observed fact no sentence states and
  renders the sentence for the rest, every observed fact is documented where collectors read it,
  every outcome measurement agrees with its sentence, every readiness has a note, the published
  palette
  is present with the accent defined once, the workbench capability grants listen without emit,
  and every catalog tool has a medallion. Each of these was checked against a negative control:
  breaking the thing it guards makes it fail.
- Outcome-language oracles cover every success sentence, every refusal sentence, workspace reason
  mapping, number grouping, safe next steps, sentence/measurement agreement, and the unchanged
  `Observed` JSON round trip. Executor tests prove the browser seam records landing facts without
  counts and the completion path still combines host/readiness with the outcome measurement.
- The complete desktop-workbench change, from its starting revision through the live-monitor
  rebuild, has an empty diff under `crates/mcp-connector`, `crates/browser-connector`,
  `crates/bridge`, and `extension`. The later demand-start lifecycle intentionally changes the
  bridge and both connectors at their connection-lifetime seam; the extension remains unchanged.

## Visual language and monitor content

- Both surfaces share one motion vocabulary. The workbench names its beats as `--beat-*` tokens
  taken from the renderer's frozen `visualIdentity`, so a treatment meaning the same thing in the
  page and the window keeps the same tempo.
- The in-page effect registry (`TRANSIENT_EFFECTS`) owns both reduced-motion enrollment and each
  treatment's beat, and teardown derives from the beat. No effect lifetime is hand-picked.
- The renderer stylesheet is static CSS: identity arrives once as custom properties, leaving only
  the token block and the generated reduced-motion selector interpolated.
- A click describes itself end to end. `ClickShape { clicks, button }` rides on
  `PresentationSignal`, and the renderer draws one ring per click, dashed for a secondary button.
- Audit records and workbench history carry the Ghostlight-authored `summary` and a measured
  `duration_ms`, so every row states what happened and how long it took.
- Outcome language now leads with the action and names the governed place. Browser action receipts
  return the role and accessible name of the physical element in the same effect response, without
  a describe round trip. The orchestrator narrows raw roles to a closed noun, normalizes and bounds
  names to 80 characters, and produces sentences such as `Clicked the "Save" button on
  example.com`. `preserve_target_names` defaults to true; false in either authority layer removes
  names monotonically and leaves `Clicked a button on example.com`. Editable values never supply a
  name. A refused explicit navigation adds only its normalized host to the existing observation
  shape, never its path, query, fragment, or value. Rendered label whitespace is normalized before
  the name is retained, so visually separate label fragments cannot collapse in the audit sentence.
- Per-action observation is built, at the seam it was designed for. See
  [`design/action-observations.md`](design/action-observations.md).
  - `language/outcome.rs` owns `Outcome`, `Refusal`, `WorkspaceReason`, and
    `Observed { host, readiness, count, width, height }`. Every successful completion requires an
    `Outcome`, so its Ghostlight-authored sentence, safe next steps, and named measurements cannot
    drift into separate call-site strings.
  - `Executor::dispatch` remains exhaustive over browser outcomes and gathers host/readiness keyed
    by invocation. `Outcome::observed` supplies counts and capture sizes from the same value that
    authored their sentence. The one completion path merges the outcome over the seam and clears
    the registry.
  - The host and optional governed action-target name are the deliberate line. Never the path,
    query, fragment, selector, target handle, entered value, or arbitrary page text. A capture reports its
    pixel size, a wait reports how long it waited and which condition it waited on, and a read
    reports how many words it read.
  - A count is recorded only where the Ghostlight-authored sentence beside it names what was
    counted, so the count needs no per-tool wording table on the surface. Those summaries now state
    their measurement: "Read 1,240 words from example.com.", "Filled 3 fields on example.com and
    submitted the form.", "Found 7 matches on example.com.", "Captured the viewport at 1280x720."
  - Rows always render the outcome sentence and add a readiness note where a document never
    settled. They no longer guess between host and measurement, because the orchestrator already
    chose which register the sentence uses. The hero renders the same sentence and carries no host
    chip: the sentence names the host, so a chip would say it twice. Readiness is the one observed
    fact no sentence states, and it is the only one the surface reads structurally. The host is
    guarded where it is collected, in
    [`guides/siem-integration.md`](guides/siem-integration.md), because that guide is what a
    person configuring a collector reads.
  - The audit stays content-minimized. `InvocationResult::facts` still carries page text and full
    URLs to the model; the observation is a separate closed type so there is no shortcut between
    them. The bounded action-target name exists only inside Ghostlight's terminal summary and may
    be removed by governance.
    [`guides/siem-integration.md`](guides/siem-integration.md) now documents `summary`,
    `duration_ms`, and `observed`, and states the host exception where it used to claim that no
    host is ever recorded.

- A service session used to outlive its connection whenever that connection ended badly. The
  request loop propagated read errors, oversized frames, and malformed lines out of the handler
  before the release ran, and an unowned workspace has no owning process for the reaper to check,
  so nothing could collect it afterwards: the workspace and every tab it held survived until the
  service restarted. A live workbench showed 17 sessions against 5 connectors. The teardown now
  runs on every exit path, guarded by a test that fails when the old early return is put back.
- The workbench connections bar groups its chips by client label, with a tally when one client
  holds more than one session. The sessions array itself is untouched, because history attribution
  resolves a single workspace to its client by id.

- The workbench surface is hardened and, for the first time, actually executed by a test.
  `node tests/workbench-surface.mjs` runs `app.js` against a minimal DOM with one panel broken on
  purpose and asserts the window still comes up, the failure is visible, the rest of the pass
  continues, and the broken panel is retried rather than memoised as done. Every other guard over
  this window reads its source as text, and none of them could tell that the window never started.
- Four fragilities behind that failure are fixed: the element table is derived from the document
  instead of hand-listed; boot is one ordered sequence that installs its own recovery first;
  wiring is an isolated step rather than loose statements ahead of boot; and a render failure is
  reported as itself instead of as a lost connection.

- The workbench surface is rebuilt around its seams. It was one 1045-line file where vocabulary,
  cache, rendering, transport and wiring were the same thing; it is now `ui/lib/words.js` and
  `ui/lib/entries.js` (pure), `ui/lib/transport.js` (the only caller of the orchestrator),
  `ui/lib/store.js` (the cache and its only writer, announcing a closed set of seven change
  kinds), `ui/lib/view.js` (the only thing that touches the document), and `ui/app.js` as a
  296-line composition root. Data flows one way: transport brings a snapshot, the store folds it
  in and announces, the view draws what it is handed. A view that cannot fetch cannot fail on a
  missing snapshot; a store that never sees the document cannot be corrupted by a paint.
  Guards hold the seam: words, entries, store and transport fail the build if they contain
  `document.`, `window.` or `el[`, with the view as the negative control.

## Executor decomposition

The executor-split batch is complete through `4d633fbc`.

- The operation families now live in `work/reading.rs`, `work/navigation.rs`,
  `work/recording.rs`, `work/pointer.rs`, `work/forms.rs`, and `work/sequence.rs`.
- `work/mod.rs` now contains the dispatch spine, shared execution infrastructure, free helpers,
  private shared types, and the unchanged test module. It fell from 5,824 to 3,255 total lines,
  with its production portion falling from roughly 4,200 to 1,633 lines.
- Each family landed as one pure-move commit. Every task independently passed formatting,
  warnings-denied Ghostlight Clippy, and all 226 orchestrator library tests without test edits.
  The durable task record is [docs/tasks/executor-split/LEDGER.md](tasks/executor-split/LEDGER.md).

## Owed

- ADR-0121 Decision 3's always-available policy explain operation still exists only as a CLI command
  over a file path. The 22-tool catalog has no policy tool, so the model cannot ask what current
  authority permits, even though a person now can.
  [ADR-0122](adr/0122-readable-policy-destination-and-authored-user-layer.md) Decision 9 defers the
  model-facing rendering to a later ADR; the projection it would use exists.
- A row that never settled reads its readiness as a parenthetical. Colour would carry it better
  than words: the duration cell already has a running and a blocked treatment, and an unsettled one
  would be found while scrolling instead of read for.
- ADR-0105 stages 2 and 3 are blocked on an owner decision, recorded in that ADR's amendment.
  Identifying the socket peer and verifying a signature are both raw Win32 FFI, and the workspace
  sets `unsafe_code = "forbid"`, which no scoped `#[allow]` can override. The choice is to relax
  that invariant for one audited module or to depend on a wrapper crate on a security-sensitive
  path. Until then the channel stays attribution: the `channels` switch decides whether an intake
  may open a session, which is a weaker claim than knowing who is calling.
- `crates/mcp-connector` still has its own copy of the service handshake and did not adopt
  `ServiceClient`. One home exists now; the connector should move to it.
- The extension stylesheet could move to its own module now that it is static. Lowest value of the
  maintainability steps; needs about eight test assertions reworked.
- GIF quality remains deferred. The vendored encoder quantizes each frame to its own 256-colour
  palette with no dithering, which suits flat interface pixels and not photographs. Overlays,
  action tagging, and perceptual palettes are still unbuilt. Output size is no longer the pressure
  it was: a browser-local save may spend 16 MiB, and anything over its budget is thinned rather
  than refused.
- `origin/main` still carries 0.8. Deciding when the 1.0 line is promoted is a release decision.
- ADR-0084's complete browser-window attention routing remains deferred; only the narrow Chromium
  slice is implemented.

## Release gates still requiring an owner or release environment

- Preserve build-only candidate `fd86403` and its verified manifest while the remaining live gates
  run. CI `31920645118` and candidate workflow `31920647296` are green; all 19 candidate files have
  repository-, workflow-, source-, and ref-bound provenance.
- Use that provenance-attested bundle to verify clean install, public-0.8 upgrade, and uninstall on
  clean Windows and Linux machines. The Windows development-host package lifecycle and headless
  Debian/Ubuntu smokes do not replace those release-environment rows.
- Complete interactive native-window, tray, and notification smoke tests on each platform. The
  automated environment verifies native build and failure containment but does not expose its GUI
  desktop to the test runner.
- Verify demand-start, direct workbench activation, and deploy quiesce from each clean
  platform installation.
- Repeat the now-passing development-host browser-job matrix against the provenance-bound Debian
  candidate and matching store adapter on Ubuntu GNOME Wayland. The unpacked-adapter CachyOS run
  does not replace that release-environment gate.
- Reconcile release metadata, public status, store submission, compatibility, distribution, and
  the final public documentation only when the 1.0 artifacts exist.
- Use the Chrome Developer Dashboard for store submission. Repair API V2 access only if its optional
  automation is worth the effort; it is not a release gate.
- Re-run the Linux lifecycle and visible-browser policy matrix on the current policy-restoration
  revision using the `test-01` development host.
- Publish the candidate-bound `ghostlight@1.0.0` tarball only after its six raw GitHub assets are
  observable. Then update `server.json` and publish the MCP Registry record; it must never point a
  1.0 record at the public 0.8 launcher.
- Keep the checked 0.8 recovery matrix aligned as current proofs move. Its remaining live-package
  and visible-browser rows are covered by the release gates above.

## Canonical 1.0 sources

- Product intent: [`1.0/INTENT.md`](1.0/INTENT.md)
- Model-facing language: [`1.0/LANGUAGE.md`](1.0/LANGUAGE.md)
- Architecture: [`1.0/ARCHITECTURE.md`](1.0/ARCHITECTURE.md)
- Acceptance: [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md)
- Desktop decision: [`adr/0102-integrated-desktop-workbench.md`](adr/0102-integrated-desktop-workbench.md),
  including its 2026-08-11 amendment for the live monitor, the published palette, and the
  three-destination workbench.
- Outcome language decision:
  [`adr/0103-language-owned-outcome-voice.md`](adr/0103-language-owned-outcome-voice.md).
- Demand-start and single-engine decision:
  [`adr/0104-demand-start-single-engine-and-workbench-activation.md`](adr/0104-demand-start-single-engine-and-workbench-activation.md).
- One backgrounded desktop-startup decision:
  [`adr/0112-one-minimized-desktop-startup.md`](adr/0112-one-minimized-desktop-startup.md), amended
  for Linux by [`adr/0118-recoverable-linux-workbench-startup.md`](adr/0118-recoverable-linux-workbench-startup.md).
- End-to-end browser availability decision:
  [`adr/0113-end-to-end-browser-adapter-liveness.md`](adr/0113-end-to-end-browser-adapter-liveness.md).
- Plural browser adapters and routing decision:
  [`adr/0114-plural-browser-adapters.md`](adr/0114-plural-browser-adapters.md).
- Packaged native-host lifecycle decision:
  [`adr/0115-packaged-native-host-lifecycle.md`](adr/0115-packaged-native-host-lifecycle.md).
- Supported operating-system scope:
  [`adr/0116-windows-and-linux-platform-scope.md`](adr/0116-windows-and-linux-platform-scope.md).
- Effective harness configuration resolution:
  [`adr/0117-effective-harness-config-resolution.md`](adr/0117-effective-harness-config-resolution.md).
