# Break 2.0: install journey ledger

The journey lane carries [ADR-0200](../../adr/0200-the-install-journey.md): one install journey in
three steps (This computer, Your browser, Your agent) across the extension's setup page, the popup,
the window's setup view and the install output. It follows the seams ledger ([seams.md](seams.md))
and the shell ledger ([shell-m2.md](shell-m2.md)); ADR-0199 wins over ADR-0197 where they differ and
ADR-0200 wins for the journey. The wire is [adapter-protocol-4](../../design/adapter-protocol-4.md).
The ground rules are the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md) plus the gate and rules
below. Slices, in order: j01-adr, j02-kit, j03-show-event, j04-setup-page, j06-popup, j07-options,
c01-carriers, c02-get-scripts, x01-store-submission, x02-release-order, j09-first-request,
j10-profiles, j11-setup-language, j11b-agent-outcomes, j12-setup-part, j13-setup-actions,
j14-window-view, j15-install-cli, j17-docs. j08-toolbar-icon is dropped (JQ1 = No). A slice whose
owner answer is Pending stops and says so
here.

## Gate

Run from the repository root (E:/repo/github/sylin-org/ghostlight) in Git Bash with
CARGO_TARGET_DIR=E:/repo/github/sylin-org/ghostlight/.target-lane-journey on every cargo command.
Run each check the slice's files require, and record in the ledger which ones ran.

- (1) Any Rust, Cargo.toml, Cargo.lock or rust-toolchain.toml change: cargo fmt --all -- --check
  (fix with cargo fmt -p <package>); cargo clippy --workspace --all-targets -- -D warnings (always
  workspace-wide); cargo test -p ghostlight-service, or cargo test --workspace when crates/bridge,
  crates/relay, crates/edge, crates/win-peer or any manifest changed.
- (2) Model surface changed on purpose (j11b only): GHOSTLIGHT_CATALOG_UPDATE=1 cargo test -p
  ghostlight-service the_catalog_matches and GHOSTLIGHT_CATALOG_UPDATE=1 cargo test -p
  ghostlight-service work::tests::golden, then review the golden diff and run the tests again
  without the variable.
- (3) extension/ or tests/golden/ changed: npm test in extension/.
- (4) Page assets changed: node --test
  "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js", plus cargo
  test -p ghostlight-service and npm test in extension/.
- (5) extension/ or scripts/package-extension.ps1 changed: pwsh -NoProfile -File
  scripts/package-extension.ps1 -OutputPath .tmp/lane-journey/extension.zip -Force.
- (6) crates/orchestrator/ui or the workbench test fixtures changed: node
  tests/workbench-surface.mjs and node --test tests/workbench-transport.test.mjs
  tests/workbench-settings.test.mjs.
- (7) packaging/ changed: npm test --prefix packaging/npm and node --test
  packaging/mcpb/test/launcher.test.js.
- (8) tests/golden/, tests/guards/, crates/relay, crates/edge, any Cargo.toml or Cargo.lock, or any
  packaged extension file changed: node --test tests/guards/*.test.mjs (the connectors guard
  tests/guards/connectors.test.mjs and, once x01 lands, the extension freeze guard).
- (9) Changed runtime JavaScript or .mjs that no test runs: node --check <file>. Changed .sh: sh -n
  <file>. Changed .ps1: a parse-only check, pwsh -NoProfile -Command
  "[System.Management.Automation.Language.Parser]::ParseFile('<file>',[ref]$null,[ref]$e) >
  $null; if ($e) { $e; exit 1 }".
- (10) Always: pwsh -NoProfile -File scripts/check-repository-integrity.ps1 (ASCII and links).

Process hygiene: never search a whole drive (search only inside the repository or the session
scratch directory); never run a Ghostlight binary, the dev loop, an installer, a get script, a
journey under tests/ that starts Ghostlight, or a browser (the coordinator runs every live check);
never touch ~/.ghostlight; never read local/, private/ or saps/; leave no process running when the
slice ends (no background job, watcher, server or node process you started may outlive the slice).

## Rules

1. Never run a Ghostlight binary, journey, install, get script, the dev loop or a browser. The
   coordinator runs every live check; record each under Owed live checks.
2. Never submit, publish, push or tag. Never edit `CHANGELOG.md` or `docs/STATUS.md`.
3. Commit only your own paths: `git add -- <paths>`, then `git commit -s -m '<subject>' -m '<body>'
   -m '<the Co-Authored-By trailer the coordinator gives>' -- <paths>`. Retry while
   `.git/index.lock` exists. Never `git add -A`, `git add .`, `git commit -a`, stash, reset,
   `checkout --`, clean or rebase.
4. Another lane rewrites files under `tests/` in this checkout. If a file you must edit there has
   uncommitted changes that are not yours, wait and tell the coordinator.
5. ASCII only. A module doc on every new module and a doc on every public item whose name does not
   state its contract. Person-facing service sentences live in `crates/orchestrator/src/language/`.
6. Tests are behavioral, never source-text pins (ADR-0197 Decision 24).
7. Never read `local/`, `private/` or `saps/`. Never copy from `reference/`.
8. After x01, a packaged extension file changes only with a recorded owner decision to withdraw and
   resubmit.
9. Update this ledger in the same commit as the slice: a Landed row, RESUME HERE, owed live checks
   and every place the tree differed from the spec. Line numbers in specs drift: find code by name.
10. Never stage `docs/testing/independent-user-delight-2026-10-02.md` or
    `docs/testing/quiet-success-local-deployment-2026-10-02.md`.

## Owner answers

Q1 and Q2 were answered 2026-10-15 (see [seams.md](seams.md) Notes). The owner delegated JQ1 to
JQ11 to the coordinator on 2026-10-16 with one test, "what would be delightful for the user?", and
ruled JQ2 directly. The full wording is in ADR-0200.

| Question | Answer | Date |
|---|---|---|
| Q1 the install-time page | The extension's own bundled `setup.html`, opened only when its first connection finds the service absent. | 2026-10-15 |
| Q2 the setup page's command | Get-script one-liners, no Node.js. | 2026-10-15 |
| JQ1 grey toolbar icon while Ghostlight does not answer | No. The badge and the popup already show it. | 2026-10-16 |
| JQ2 embedded page-asset fallback (Chrome and Edge) | No. The page code is local, so the store's remote-code answer stays No (owner ruling). | 2026-10-16 |
| JQ3 host permissions | Drop http and https host permissions if the live check shows nothing needs them. | 2026-10-16 |
| JQ4 frozen URLs and the pre-submission test release | `irm https://sylin.org/ghostlight/get.ps1 \| iex`, `curl -fsSL https://sylin.org/ghostlight/get.sh \| sh`, documentation link https://sylin.org/ghostlight/. Both redirect to the scripts on `main` (ADR-0200 amendment of 2026-10-16); the release does not attach them. The website redirect is the owner's to publish. A pre-release is proved with a GitHub prerelease (`v2.0.0-rc.1`, never latest) installed by version: `GHOSTLIGHT_VERSION=2.0.0-rc.1` with the script read from the rc tag, on Windows PowerShell 5.1, PowerShell 7 and a Linux shell (ADR-0200, amendment "reviewers install a pinned prerelease, and the cutover waits for approval"). The exact strings are proved at the cutover, after approval. | 2026-10-16 |
| JQ5 a Watch tab during setup | Shows the ordinary workbench. | 2026-10-16 |
| JQ6 installed but not running | "Open Ghostlight on this computer." and where it is (Start menu or Applications), on the setup page and in the popup. | 2026-10-16 |
| JQ7 scope of 2.0.0 and the cutover | 2.0.0 waits for the service half (j09 to j15). | 2026-10-16 |
| JQ8 Add and Open <browser> | Ghostlight starts that browser. | 2026-10-16 |
| JQ9 Edge | Add from the Chrome Web Store until an Edge listing exists. | 2026-10-16 |
| JQ10 window on the first no-browser refusal | None. The agent's answer carries the link. | 2026-10-16 |
| JQ11 Skip for now | Kept, quiet; the tray brings setup back until done. | 2026-10-16 |

## RESUME HERE

j15-install-cli has landed. Next: j17-docs. Rule 8 still binds: a packaged extension file changes only with a recorded owner decision to withdraw and resubmit. The release order is unchanged: release candidate as a prerelease, pinned install proof, staged submission, approval, then the one-sitting cutover (ADR-0200, amendment "reviewers install a pinned prerelease, and the cutover waits for approval"). j17 reconciles the documents that still describe the old install output and the remote walkthrough page: `docs/trust/data-flows.md` ("No vendor page opens" is now true again), `docs/1.0/ARCHITECTURE.md` (the state directory list names "the extension handoff marker", and the session-end paragraph says "the install handoff opens no page"), the guides and `docs/STATUS.md`. A real install now prints `language::setup::install_lines` and the lines that need a person; mechanism prints only with `--dry-run`.

## Landed

| Slice | What | Commit |
|---|---|---|
| j01-adr | ADR-0200 is filed and indexed with Decisions 1 to 16 and the owner answers above. The ADR index marks ADR-0070, 0091, 0095, 0126 and 0177 as amended by it. The adapter-protocol-4 note gains a marked amendment superseding the section 7 rows on publishing after services spread and the unchanged level-3 path. This ledger starts; `docs/tasks/README.md` and the seams ledger point here. | see `git log` |
| j02-kit | `extension/kit.css` is the one stylesheet for the extension pages: the sylin.org night tokens and base rules under `.gl-page`, the kit components, the setup layout, the companion with its two-segment stepper, and the popup. `scripts/package-extension.ps1` ships `kit.css` and `icons/ghostlight-mascot.png` and throws when a staged page names a local file the package lacks. The source-grep test for `setup.html` is deleted. | see `git log` |
| j03-show-event | Protocol 4 gains one browser-level event, `show_requested`, with no fields: the golden vocabulary and fixture, `extension/lib/protocol.js`, the worker (`show_ghostlight` page message, `requestShow()`, refused while the link is down), `wire::Event::ShowRequested`, `BrowserEvent::ShowRequested`, and `ServiceBrowserEvents` calling `WorkbenchFacade::reveal(None)` and ignoring the answer. The adapter-protocol-4 note carries the row and the browser-level sentence. | see `git log` |
| j04-setup-page | `extension/setup.html` is the kit page: the top bar with the wordmark and the Documentation link, the companion (ghost, lantern, stepper, step names) and the one column of eyebrow, heading, command, live line and fallback. `extension/lib/setup-view.js` (new, pure) holds every word and command and `view()` and `fallbackDue()`; `extension/setup.js` (new) places them, polls `ui_snapshot` every 1500 ms, plays the meeting once (`ghostlight.setup_met`) and sends `show_ghostlight`. The popup and options setup routes always open the bundled page and both remote URL literals are gone. The packager ships `setup.js`. The tests that pinned `setup.html` text are deleted; `extension/tests/setup-view.test.js` covers the pure module. | see `git log` |
| j06-popup | `extension/lib/popup-view.js` (new, pure) holds every popup word, `popupView()` and `connection()`. `popup.html` is the kit page (mascot at 100 px, status block, actions, Settings link) and `popup.js` places the view, redrawing only when it changes. The captions switch, the notes, the setup paragraph, the debugger line and the version are gone. The held badge is the accent `#5eead4`, and `worker.js` sets the badge text colour `#08110f`. `kit.css` gains the popup note, quiet-button and problem rules. The tests that pinned popup text are deleted; `popup-view.test.js` and `state.test.js` are new. | see `git log` |
| j07-options | `extension/options.html` is a kit page (`<html class="gl-page">`, `<body class="gl-stage options-page">`) linking `kit.css` then the new `extension/options.css`, which holds the cards, rows, switches and footer on kit tokens. The connection card's pill and line come from `GhostlightPopupView.connection()`: `gl-pill--ok` when connected, else `gl-pill--warn`; the setup button shows whenever `needsSetup` is true, is labelled `setupLabel` and opens the bundled `setup.html`. The words pass is done (strings under Owed to the owner). `extension/ui.css` is deleted and the packager ships `options.css` instead. The options source-text pins in `onboarding.test.js` and `shared.test.js` are deleted; `popup-view.test.js` gains two connection cases. | see `git log` |
| j08-toolbar-icon | Dropped. JQ1 is No: the badge and the popup already show that Ghostlight does not answer, so there is no grey toolbar icon. Nothing was built. | n/a |
| c01-carriers | The carriers ship and run the three 2.0 images (`ghostlight`, `ghostlight-service`, `ghostlight-desktop`). Most of the slice had landed in earlier carrier fixes (see Notes); this commit finishes it. The npm never-install commands (`doctor`, `status`, `help`, `--version`, `install --help`) with nothing installed now stage and run `ghostlight-service` itself instead of a staged edge beside a staged service, so they download one image, not two. The WinGet template lists `ghostlight-service.exe` and `ghostlight-desktop.exe` as nested installer files with no alias, beside `ghostlight.exe` with the alias `ghostlight`; `winget validate` accepts it. The npm README says `npx -y ghostlight` is the MCP server and `npx -y ghostlight install` installs. | see `git log` |
| c02-get-scripts | `scripts/get.ps1` and `scripts/get.sh` download the three 2.0 images and verify them as before (checksums, and `gh attestation` when `gh` is present). They print one line after verification, `Ghostlight <version> downloaded. Checksums verified.`, or `... Checksums and build provenance verified.` when every file passed attestation. The service's `deployment install` runs with stdout discarded and errors shown, then the installed `ghostlight install` runs in the same console so its summary prints and its exit code is checked. `get.ps1` uses the call operator and `$LASTEXITCODE` for both, with no `Start-Process`. The trailing doctor line is gone; the doctor hint stays inside the two failure messages. `GHOSTLIGHT_NO_REGISTER=1` prints `Ghostlight <version> is installed in <dir>. Run 'ghostlight install' to connect your browsers and agents.` Each header carries the frozen-URL sentence and the short sylin.org one-liner. The WinGet `InstallationNotes` say to run `ghostlight install` once and again after `winget upgrade`. | see `git log` |
| x01-store-submission | The store copy is true to the tree and the package is frozen. `PERMISSION_JUSTIFICATIONS.md` says page code is part of the application installed on this computer, supplied over native messaging and checked by hash, and that `browser_execute` text is written by the person's agent; the debugger, downloads, nativeMessaging, offscreen, storage and host-permission blocks follow the code (no content scripts, no scripting API, the options page's diagnostics download, the setup flag and the session keys). `PRIVACY.md`, `STORE_LISTING.md` (setup-page sentence, assets, submission gate), `trust/faq.md`, `trust/questionnaire.md`, `LICENSING.md`, the `gifenc.js` comment, `extension/README.md` and `ACCEPTANCE.md` extension gates 3, 4 and 7 are corrected. `docs/release/store-disclosure-2.0.md` (new) holds the dashboard answers (remote code: No, per the owner ruling), the dated Chrome and Edge policy reading, JQ2's answer, the staged release order and the assets to recapture. The freeze: `tests/golden/extension-2.0.0.files.sha256` (47 packaged paths and the zip's hash) and `tests/guards/extension-freeze.test.mjs` (with `tests/guards/lib/extension-package.mjs`), run by the extension CI job. The packager now stages text files with LF line endings, so one commit packages to the same bytes on every machine. | see `git log` |
| x02-release-order | Reviewers and the owner can install 2.0 before the cutover, and the cutover waits for store approval. `scripts/get.ps1` and `scripts/get.sh` install exactly `GHOSTLIGHT_VERSION` when it is set: the release is found by its tag (`v<version>`) through the GitHub releases API, a prerelease included, and a version with no release (HTTP 404) is refused in one sentence with a non-zero exit, and any other failed lookup (a rate limit, a server error, no connection) is reported as a failure and never as an absent release. Unset, they resolve `releases/latest` as before. Everything after the release is chosen is unchanged. `tests/installer-shell.mjs` and `tests/installer-powershell.ps1` cover a requested stable and prerelease version resolving its own tag, never asking for latest, an absent or malformed version refused, and a 403, a 503 and a failed connection not called absent. `scripts/publish-github-release.ps1` takes `-Prerelease <suffix>` (tag `v<version>-<suffix>`, `--prerelease`, the same custody checks, a draft whose prerelease flag differs refused), so the rc is published with the release tooling. `docs/release/store-disclosure-2.0.md` has the new release order with the exact commands (rc prerelease, pinned proof, staged submission with the drafted "Notes for certification", approval, then the cutover in one sitting) and says the rc carries the 2.0.0 build, so its binaries report 2.0.0. ADR-0200 gains the amendment "reviewers install a pinned prerelease, and the cutover waits for approval"; the ADR index row says so. | see `git log` |
| j09-first-request | Setup is remembered. `state/setup.json` is `{"first_request_ms": <u64>}` (`install/setup_state.rs`: `SetupRecord`, `is_complete`, `read`, `record_if_absent`, written with `record::atomic_replace`; read leniently, and a file that exists but does not parse counts as complete). `workbench/setup.rs` holds `SetupProgress` (`load`, `is_complete`, `finished`, `observe`, `on_complete`): the first non-child record that is on the MCP channel, allowed, `Status::Succeeded.stored_name()` and has a browser writes the record once and remembers the workspace in memory. `WorkbenchProjection::attach_setup` and `record_with_provenance` feed it, the service attaches it from `Layout::setup_record()`, and `WorkbenchFacade::setup()` hands it to j12. `Status::stored_name()` is the one rendering of the stored status, now also used by `work/receipt.rs`. The break migration writes the record (dated at the migration) when it migrates a 1.x home with no record, and the 1.x `extension-handoff-v1` marker is no longer moved but deleted. | see `git log` |
| j10-profiles | The service can tell "extension added, browser closed" from "extension missing". `install/native_host.rs`: `BrowserSpec` gains `short` (Chrome, Edge, Brave, Chromium); `NativeHostRegistry::user_data_directory(browser_id)` (the vendor path under `LOCALAPPDATA` plus `User Data` on Windows; the XDG config home plus `linux_directory` minus `NativeMessagingHosts` on Linux) and `browser_identities() -> Vec<BrowserIdentity { id, name, short }>` in table order. `install/browser_profiles.rs` (new): `ExtensionProfile { directory, name }`, `ProfileScan { profiles, with_extension }`, `scan_user_data(user_data, extension_id)` (never errors; reads at most 8 MiB of `Local State` `profile.info_cache` in file order, uses only keys of 1 to 64 characters that are one plain path component, shows names cleaned and cut to 40 characters, and falls back to an unnamed `Default`, counted when its directory exists) and `scan(registry)` for every browser whose executable is found, returning `BrowserProfiles { browser, scan }`. Read-only, local, nothing written, logged, stored or sent. Nothing uses the result until j12. | see `git log` |
| j11-setup-language | `crates/orchestrator/src/language/setup.rs` (new, pure, no I/O, shaped like `readiness.rs`) holds the words of the setup view and of the install summary. `compose(&SetupFacts) -> SetupView` resolves the stage (Done, Retired, Computer, Browser, Agent) and builds the headline, steps, code panels, rows and live lines; `RowAction`, `SetupAction` (tag `kind`: `add_in_browser`, `open_browser`, `get_chrome`, `connect_agent`, `review_agent`) and `Pending` carry the actions and the words the page shows after a click. `install_lines(&InstallFacts)` is the whole success output of a real install, with exactly one Next line while setup is incomplete. `STORE_LISTING_URL`, `PROMPT`, `TAGLINE` and the step names are constants. The module doc classifies which deployment lines print beside the summary and which only in `--dry-run`. Nothing calls it yet (j12 and j15 do). | see `git log` |
| j11b-agent-outcomes | The agent's answers say where the extension comes from. `language/outcome.rs` gains `ExtensionAdvice` (`Plain`, `AddFromStore`; `resolve(setup_complete, extension_in_a_profile)`), carried by `Refusal::BrowserStartupManual` and `Refusal::BrowserRecoveryFailed`. With `AddFromStore` the no-browser answer gains one next step and the `ExtensionAbsent` step is replaced by it: ask the user to add Ghostlight in Browser from `language::setup::STORE_LISTING_URL`, or to open the Ghostlight window for the setup steps. The executor (`work/mod.rs`) decides when the refusal is built: `WorkbenchProjection::setup()` (new) says whether setup is complete, and only while it is not does `extension_probe` run the j10 `browser_profiles::scan`. No attached setup progress means no advice. The incompatible-extension words are `language::adapter::ADAPTER_INCOMPATIBLE` ("Ghostlight in Browser is out of date. It updates itself within a day; to update now, open chrome://extensions, turn on Developer mode and click Update.") and the agent's next step for the contract refusals, both from `adapter::UPDATE_NOW`. `docs/1.0/LANGUAGE.md` and ADR-0200 Consequences carry the wording. | see `git log` |
| j12-setup-part | The setup part is live. `Part::Setup` (token `setup`) is the last of `Part::ALL`; the window's projection is the `SetupView` and a watcher's omits it (`Projection::shows`, JQ5), so `GET /api/snapshot` leaves it out for a watcher and `parts=setup` as a watcher answers an empty `parts`; Closed gets nothing, as for every part. Screens learn of a change from `part_changed` with `part` `setup` and the tag of the view (`HintedPart::Setup`; a watcher's hint carries no tag). `workbench/setup.rs` holds the facts and `setup_view()`: complete and finished from `SetupProgress`; platform from `cfg!(windows)`; `finish_on_this_computer` when the native-host report has a private view; each browser's form from the report's package (`NotChecked` and an unreadable report fall back to whether the executable is found), its profile from the j10 scan, `connected` by name; an unknown connected browser is a connected `Chromium`; agents from the cached roster (`NotDetected` is no row; `seen` when a live session's client label matches the roster label or id, any case); the MCP command from the roster's `connector_command`. A complete setup reads nothing from the machine; an incomplete one reads the report and runs the scan each time the part is built. `workbench/live.rs` builds the part with every look, but only when a connected browser's name or a session's client label changed, or a caller said setup changed; it announces the part only when its tag differs. `WorkbenchFacade::attach_setup` wires `on_complete` to a rebuild, and every publication of the integrations part (a rescan or a client act) also rebuilds setup. `WorkbenchFacade::setup_view()` builds the part on demand; it is not in `WorkbenchSnapshot`. The `SetupMachine` trait is the one seam to the computer, with a fake in `setup::test_support`. | see `git log` |
| j13-setup-actions | The setup view's browser steps are service acts (JQ8 = launch). `WebAct::SetupAct` (key `setup_act`, window only) sits behind `POST /api/setup/act` (`WatchAccess::NeedsWindow`, `SMALL_BODY`): the body is a `SetupAction`, the answer the fresh `SetupView`. `workbench/setup.rs` `perform(machine, action)` refuses first while the session is ending, then carries out `add_in_browser { browser }` (the browser's executable, with `STORE_LISTING_URL` as its only argument) or `open_browser { browser, profile }` (`--profile-directory=<profile>` as its only argument, none without a profile). The browser must have a found executable now (Edge included, JQ9) and a profile must be one the fresh scan returned for that browser; every other kind of step, and anything not offered, is refused with `WorkbenchError::SetupStepNotOffered` ('That setup step is no longer offered.'); the other two refusals are `SetupSessionEnding` and `SetupBrowserNotStarted`, all three in `language/workbench.rs`. `SetupMachine` is the launcher seam: `browser_found` became `browser_executable`, and it gains `session_ending` and `start`; `ThisComputer::start` proves a graphical user session, then starts the program with `relay os::spawn_detached` (no inherited handle) in the user's home directory (`NativeHostRegistry::user_home`). The fake `Computer` records every start and starts nothing. `WorkbenchFacade::setup_act` and `Live::perform_setup_act` wire it. `WorkbenchDestination::GetChrome` (`get_chrome`, `https://www.google.com/chrome/`) is the 13th destination, reached through the link table and `/api/open`. No state is kept in the service: the page words what it waits for (j11). | see `git log` |
| j14-window-view | The window has a setup view. `ui/lib/setup-view.js` (new, UMD, no fetch, no decisions) draws a `SetupView` word for word into the `#setup-root` section of `ui/index.html` (companion with the mascot at 200 px, stepper and names on the left; eyebrow, headline parts, lead, code panel, live line, agent rows and the second panel on the right) using the extension kit served at `/extension/kit.css` and `/extension/ghostlight-mascot.png` (`workbench/web/assets.rs`: a second macro arm for extension files, and a test that the served bytes are the extension's own). It exports `visible(part, {skipped, shown})`, the pure `transition(previous, next, shownBrowserMs)` (`meet`, `finish` or none) and `create({...})` with `open`, `update`, `close` and `clearPending`; time is injected (`schedule`, `cancel`, `now`). The meeting plays only after the browser step has been on screen for `MEET_AFTER_MS` (2000), swaps in the agent step at 1900 ms and asks the window to come forward once when `document.hasFocus()` is false; the completion plays alone (even beside a connection), shows Ready, the new heading and the tagline, and after 6000 ms the page goes to `#monitor`; reduced motion changes the same content at the same moments with no animation class. A click on a row posts `/api/setup/act` (Add, Open), opens `get_chrome` through the link table, posts the integrations install act (Connect) or goes to `#integrations` (Review), and the row and the live line show the step's pending words until the next part update changes that row. `PageAddress::Setup` (`setup`) is on both sides (the agreement test passes); a browser's show request reveals at it while setup is incomplete (`WorkbenchFacade::show_address`). `app.js` decides the view per part update (Control only; a watcher gets no part, so the workbench), looks for the agents once on open, treats any other address as a skip for the life of the window, and brings setup back on a reveal at `#setup`. `lib/transport.js` gains `setupPart()` and `setupAct(action)`; `lib/words.js` the address and three words (Copy, Copied, Skip for now). `tests/workbench-setup-surface.mjs` (a fake document built from the real markup, a clock and a timer list the test moves) and `tests/workbench-setup-fixtures.mjs` (shared with the preview server) hold 71 checks run by `tests/workbench-surface.mjs`; the preview server takes `?setup=computer|browser|agent|done` (default retired). | see `git log` |
| j15-install-cli | The install shows the window and says only what is next. `cli/setup.rs`: a real install prints `language::setup::install_lines` (built by the pure `install_facts` from a `Collected` the run fills: the agents with entries and the ones changed in this run, `needs_new_terminal`, the MCP command) and the lines that need a person (a browser registration that needs attention or is owned elsewhere, or every chosen browser when none is usable; a command, documentation or Applications entry that needs attention; a client that needs attention or failed; a stray that would not stop; a version directory that stayed; what an app container kept private). Mechanism (the connection report, `Command:`, Documentation, Applications entry, Start-menu and uninstall-key lines, one registration message per client, stopped strays, removed version directories) prints only with `--dry-run`. `should_show_window(dry_run, no_open, automated, install_usable, in_container, setup_complete)` is a pure function; when it holds, `show_window` runs the path `ghostlight open` uses (`desktop::open_with(&Detached)`), and its result is `window_shown` (a failure counts as not shown and prints nothing). The control operation `show` now reveals at `WorkbenchFacade::show_address()` (the browser's Open Ghostlight and the tray already did), so the window opens at setup while setup is incomplete. `workbench/setup.rs` gains `browsers_here` (the reads `setup_view` made, now shared) and `browsers_on_this_computer()` for the summary. `install/handoff.rs`, its `pub mod` and `EXTENSION_INSTALL_URL` are deleted; `language::workbench::stray_stopped_line` goes with the line it worded. `--no-open` help reads "Do not open the Ghostlight window" in `parse.rs` and the three Linux completions. | see `git log` |


## Owed live checks

- j01: none. Docs only.
- j10: none. Nothing uses the scan until j12; the first live check is the setup part reading a real Chrome, Edge or Brave profile (an extension added from the Chrome Web Store, browser closed) on Windows and Linux.
- j12: the setup part on a real machine: in a fresh 2.0 home, `GET /api/snapshot?parts=setup` from the window shows the Browser stage with an Add row for the installed browser; with the extension added from the store and the browser closed, the row reads "Open <browser>" (the j10 scan on Windows and Linux); when the extension connects the stage becomes Agent and the stream carries `part_changed` for `setup`; a client written by `ghostlight install` reads "Restart <label> to finish." until its first session, then Connected; the first successful agent request turns the part Done and the stream announces it once; the same machine after a restart shows Retired. Inside an app container (Claude Desktop's Store package) the stage is Computer.
- j13: Add opens the store listing in Chrome, Brave and Edge (the named browser, not the default one), from a real service on Windows and Linux; Open <browser> opens the named profile of a closed Edge (and Chrome, Brave); Get Google Chrome opens the default browser; no browser starts during sign-out (Windows and Linux); the browser started from the window does not hold the service's pipe or socket (quit the service while the browser stays open, then start it again); on Linux the browser starts when the service was started by systemd or a launcher with a thin environment.
- j14: the preview server in each state (`node tests/workbench-preview-server.mjs`, then `/?setup=computer`, `browser`, `agent` and `done`; Add or Open turns into the agent step after four seconds, Connect turns a row into the restart line, `done` finishes a few seconds after load). On test-01 after a fresh install: the window opens at the browser step; when the extension connects the meeting plays (the row turns done, disc 02 fills, the lantern flares) and the agent step arrives 1.9 s later; with the window behind another it comes forward once (Windows and Linux); an extension-first install shows the agent step at rest and no meeting; the first agent request shows Ready and the tagline, then the monitor after six seconds, and the next start opens on the workbench; Skip for now goes to the monitor, the extension's Open Ghostlight brings setup back, and so does closing and reopening the window from the tray; a watcher in a browser tab sees the ordinary workbench; reduced motion (system setting and Settings, Motion); a 390 px window (one column, the instruction first, the copy button below the command); the look of the mascot at 200 px and the companion column at 296 px in the 960 x 648 window; the page loads `extension/kit.css` and the mascot under the page's content security policy (no inline style is used); the tray Open (menu entry and a left click on the icon) of a window that skipped and is still open, behind other windows or minimized, comes forward at setup while setup is incomplete, and at the workbench once it is done; on Linux the window is built once and the page loads once (see Notes, j14, 8).
- j11: none. Pure words; the first live check is the window's setup view (j14) and the install output (j15).
- j15: `ghostlight install` in a fresh Windows PowerShell and in a Linux terminal prints exactly the summary (version line, location, the agents line, one Next line) and opens the Ghostlight window at the browser step; `--no-open` and `CI=1` open nothing and print the store-listing form of the Next line; a second install after setup is complete prints no Next line and opens nothing; `--dry-run` prints the mechanism lines and opens nothing; an install in an app container opens nothing and names what it kept private; `ghostlight doctor --fix` opens no window; a service that is already running shows its window at setup when the install finishes (the control operation `show`).
- j11b (Demo 2): install through the plugin with no Ghostlight extension and setup incomplete, ask the agent to browse, and see the store link in its answer (no window opens, JQ10). With the extension added in a closed browser, or after the first successful request, the answer carries no link. A 1.x extension connecting to a 2.0 service shows the new update sentence in its own words.
- j02: none. The page markup that links the kit lands in j04, j06 and j07; the live look is checked there.
- j03: the unpacked extension's Open Ghostlight brings the window forward on Windows and Linux (live, once j04 lands the button). The button has landed in j04; the check is still owed.
- j04: the unpacked extension in a fresh profile on a machine with no Ghostlight: the page opens once, the command copies (and `Copied` is announced), the meeting plays once when the service from the development channel answers, and Open Ghostlight shows the window. Also reduced motion, a 390 px window, and the Documentation link opening in a new tab.
- j04: measure a cold get-script run on test-01 after c02 and tune `FALLBACK_AFTER_COPY_MS` (90000) and `FALLBACK_AFTER_OPEN_MS` (120000) in `extension/lib/setup-view.js` before x01.
- j06: the popup in each state (Ready, Paused, Session ended, Not connected with the install and the start words, Update needed, recording, debugger release) on Chrome and Edge, on Windows and Linux. The key cap and `aria-keyshortcuts` follow a changed shortcut. The mascot looks right at 100 px in the lit and waiting lights. The badge text is readable on the accent.
- j04: the coordinator's check that the pinned commands run in Windows PowerShell 5.1, PowerShell 7 and `curl | sh` (see c02) is not recorded in this ledger yet.
- j07: the options page on Chrome and Edge, on Windows and Linux: it scrolls (the stage no longer clips), every switch still saves and survives a reload, the captions switch dims while effects are off, Save developer diagnostics still downloads, the connection card reads right in each state (Connected, Not connected with and without the setup button, Update needed), and the setup button opens the bundled setup page. The kit look is right at 390 px and with reduced motion.
- c02, before the cutover: the pinned install of the release candidate. Publish `v2.0.0-rc.1` as a GitHub prerelease, then run it from the rc tag in Windows PowerShell 5.1 and in PowerShell 7, `$env:GHOSTLIGHT_VERSION = "2.0.0-rc.1"; irm https://raw.githubusercontent.com/sylin-org/ghostlight/v2.0.0-rc.1/scripts/get.ps1 | iex`, and in a Linux shell, `curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/v2.0.0-rc.1/scripts/get.sh | GHOSTLIGHT_VERSION=2.0.0-rc.1 sh`. Each shows one verify line, then the install summary. A version with no release prints one sentence and exits non-zero. A failure (a development install present) shows its reason and the failing command's own output. The install summary prints in the console on Windows now that nothing is hidden. A cold-run time on test-01 for j04's fallback constants (`FALLBACK_AFTER_COPY_MS`, `FALLBACK_AFTER_OPEN_MS`). All of these are needed before x01 submits. At the cutover, after approval and after `main` advances, the exact frozen strings, `irm https://sylin.org/ghostlight/get.ps1 | iex` and `curl -fsSL https://sylin.org/ghostlight/get.sh | sh`, and the launcher restore commands.
- x01: an install of `.tmp/lane-journey/extension-2.0.0.zip` as an unpacked folder on Chrome and Edge. After the owner submits (staged), the staged package itself through trusted testers; a packed Chrome clamps `chrome.alarms` to 30 seconds, so measure the time to connect after an install. Submit only after the c02 pinned proof passes; the pinned commands go in "Notes for certification" (drafted in `docs/release/store-disclosure-2.0.md`).
- x01 (JQ3): the unpacked extension with `host_permissions` removed, on an http and an https page, covering attach, `webNavigation` events, capture, navigate, read, click, screenshot, execute and recording. No result is recorded, so the package keeps the host permissions. If nothing needs them, remove the block, the host-permission justification and its listing line, regenerate the golden, and run the check again on the final zip.
- c01: `npx -y ghostlight install` on test-01 (Windows) and on a Linux machine; then an MCP client with `npx -y ghostlight` connects. `npx -y ghostlight --version` on a machine with nothing installed downloads and runs only the service. The WinGet alias `ghostlight` finds `ghostlight-service` beside its target (the portable zip extracts all three to one folder). The Claude Desktop MCPB on Windows. The window and tray appear after each.
- j09 (after j14): a 1.x home upgraded to 2.0 on a test machine opens on the workbench, not setup (`state/setup.json` exists after the install). A fresh 2.0 home: the first successful `browser_navigate` from an MCP client writes `state/setup.json`; a `ghostlight` command-line request does not.

## Owed to the owner

- Publish the sylin.org redirects `/ghostlight/get.ps1` and `/ghostlight/get.sh` to the scripts on
  `main` (separate repository, `src/_redirects`).
- The 2.0 release order in `docs/release/store-disclosure-2.0.md` (ADR-0200, amendment "reviewers install a pinned prerelease, and the cutover waits for approval"): the rc prerelease, the pinned proof, the staged submission, approval and the live privacy text, then in one sitting `v2.0.0` as latest, `main` fast-forwarded, the exact frozen strings, and the staged publish. Not before approval.
- The Edge listing is not requested; Edge installs from the Chrome Web Store (JQ9).
- x01: the dashboard submission (staged, never published before the release order in `docs/release/store-disclosure-2.0.md` holds), the remote-code answer (No), the recaptured popup, options and setup assets, the public privacy URL carrying the new text before publication, and the decision on an Edge Add-ons listing.

- j07 words pass for the options page, for approval at Demo 1, before x01 (no setting key, default
  or behaviour changed). Before -> after:
  - Subtitle: "Local browser behavior and feedback. Governance lives in the binary." -> "Settings for this browser."
  - Checking line: "Checking the link to the Ghostlight service..." -> "Checking Ghostlight on this computer..."
  - Preserve tabs: "Keep controlled pages open as visible evidence after Ghostlight works. When enabled, agent requests to close tabs are refused even when service policy permits them. You can always close tabs yourself." -> "Agents cannot close the tabs Ghostlight works in."
  - Effects: "The phantom cursor, click ripples, scan-lines, and the rest while the agent works. Turn off for a completely quiet browser." -> "Shows the cursor, click ripples and scan-lines while an agent works."
  - Captions: "A subtitle naming each action ("Reading page", "Screenshot"). Handy when recording a demo; off for everyday." -> "Names each action in a caption as the agent does it."
  - Diagnostics: "Saves bounded connection details for this browser and shows the raw connection error. Off by default." -> "Keeps connection details for this browser and shows the raw connection error."
  - Governance card: heading "Governance and policy" and the paragraph "Which domains the agent may touch, ... a local physical interlock that can refuse closing but never grant authority." -> no heading, one line "Policy and activity are in the Ghostlight window."
  - Connection card (now the popup's words): "Connected / The agent can reach this browser." -> "Connected / Ghostlight on this computer is answering."; "Not installed here / Ghostlight is not installed on this computer yet. The extension came with your Chrome profile. Install Ghostlight here to connect it." -> "Not connected / Ghostlight on this computer isn't answering." with button "Set up this computer"; "Waiting / Waiting for the Ghostlight service. Start it, and this turns green on its own." -> "Not connected / Open Ghostlight on this computer. It is in the Start menu." (or "in your applications."); "Version mismatch / The extension and Ghostlight service cannot communicate until their adapter contracts match." -> "Not connected / Ghostlight on this computer isn't answering." with the setup button; "Update needed / Update Ghostlight on this computer." keeps its line and gains the button "Update this computer"; "Session ended / Browser access is severed. Start a new session from the toolbar popup to reconnect." -> the card reads Connected (the popup tells the session state).

## Notes

- j11: ran gate checks (1) fmt, workspace clippy and `cargo test -p ghostlight-service`, and (10) the
  integrity script. No extension, golden or page-asset file changed, so (3) to (9) did not apply.
  The tree and the spec differed in these places, and these are the choices made.
  (1) JQ8 = launch and JQ9 = yes, so there is no `open_store_listing` action and Edge gets Add like
  Chrome, Brave and Chromium: every native browser without the extension gets one Add row, primary
  for the first. `OpenBrowser` carries `profile` only when the browser has two or more profiles;
  with one profile it is none and the browser chooses.
  (2) `SetupView` has no `note` field. The spec listed one but named no sentence for it, and every
  approved sentence already has a place. j14 should not draw a note.
  (3) `StepState` has no `Off` and the screen-reader suffix ", not supported" is not carried: the
  service never runs on a computer Ghostlight does not support. `sr` is the step name plus ", done"
  or ", current step".
  (4) `Live { text, done }` is kept as specified. A composed view always has `done` false; the page
  turns it on when it swaps in `connected_line` (j14).
  (5) `connected_line` is set in the Agent stage only. The Retired view carries no words.
  (6) The install's restart line reads "then ask it" for one changed agent and "then ask one" for
  several, and the Agents suffix reads "restart it" or "restart them" by how many changed, so the
  English is right for one agent. Agents with no entry are not listed on the Agents line; when no
  agent is detected at all the MCP command line prints.
  (7) `InstallFacts` carries the browser facts, not a flag: "extension in some profile" is any
  browser that is connected or holds the extension, and the browser in the Next line is the first
  native one in row order, from the same ordering the setup view uses.
  (8) Sandboxed rows (Snap, Flatpak) show only when no browser is native; each carries Get Google
  Chrome, primary on the first. A browser with the extension counts as native.
- j11b: ran gate checks (1) fmt, workspace clippy and `cargo test -p ghostlight-service` (1789 pass),
  (2) both golden updates (the catalog and result goldens did not change: no golden samples these
  refusals; the next-step vocabulary check in `language/self_check/hints.rs` samples both advice
  values), and (10) the integrity script. No extension, page asset or script file changed, so (3) to
  (9) did not apply and the extension freeze held. The tree and the spec differed in these places.
  (1) JQ10 is answered No, so nothing calls `WorkbenchFacade::reveal`; the agent's answer carries the
  link. (2) The store step is a next step, not a change to the summary, so the summaries are as before
  and the goldens did not move. `BrowserRecoveryFailed` carries the advice for every reason but it
  changes only `ExtensionAbsent`. (3) The incompatible words exist in three places, not a Connections
  or doctor sentence: the extension's `adapter_incompatible` message, the agent's next step for
  `IncompatibleReceipt`, `BrowserAdapterOutdated` and `BrowserContract`, and `docs/1.0/LANGUAGE.md`.
  The hint checker reads `now:` as a field name, so the agent's step puts the procedure in
  parentheses. (4) The update procedure: Chrome's extension documentation (developer.chrome.com,
  "Host extensions on Linux"/update pages, read 2026-10-16) says the browser checks installed
  extensions every few hours and that the Update extensions button on the extensions page forces a
  check; that page does not mention Developer mode, which is the spec's procedure and is kept. Edge and Brave open `chrome://extensions` on their
  own extensions pages. "Within a day" is the owner's wording from the spec. (5) Tests set
  `ApplicationExecutor::extension_probe` directly, so no test scans the machine it runs on.
  (6) Review finding, fixed in a follow-up: `RecoveryFailure::ExtensionAbsent` is never
  constructed, so advice on it alone was unreachable. On the Windows default (on demand, one
  native browser) Ghostlight launches the browser, no adapter arrives without the extension, and
  recovery reports `HandshakeTimeout`. `BrowserRecoveryReason::HandshakeTimeout` now carries the
  advice too, replacing "Start the selected browser yourself" with the store step under the same
  condition; `LaunchFailed` keeps its step. The `ExtensionAbsent` variant stays unreachable and
  is left for a later slice to retire or construct. Tests: an executor-level timeout case on an
  on-demand policy (new `BrowserRecovery::set_test_launch_without_adapter`, no process starts).
  Gate re-run: fmt, workspace clippy, `cargo test -p ghostlight-service` and the integrity script.
- j01: the tree and the spec differed in these places.
  (1) The spec's dashboard line in Decision 15 ("the remote-code answer changes from No to Yes")
  is overridden by the owner ruling of 2026-10-16: the page code is local and never pulled from a
  remote source, so the answer stays No for 1.x and 2.0 and the 1.x listing is not corrected. The
  ADR still withdraws the sentence "All extension logic ships in the reviewed package" and its
  copies (`docs/legal/PERMISSION_JUSTIFICATIONS.md`, `docs/trust/faq.md`,
  `docs/trust/questionnaire.md`) because the extension works with the locally installed
  application, which supplies the page code (Edge policy 1.2.3 requires naming the dependency);
  j17 does that pass.
  (2) The ADR-0199 amendment of 2026-10-16 is titled "a start outlasts a service that is leaving on
  idle"; idle exit itself is Decision 12. The ADR cites both by those names.
  (3) Citations checked against their ADRs: 0070 D1 to D5, 0091 D3, 0126 D10 measure 2 (the
  "Second machine" line), 0177 D2 (`cli/setup.rs`, "extension walkthrough handoff"), 0095 D2
  (`--no-open`), 0196 D5 and 0199 D23. All say what the ADR claims.
  (4) `install/handoff.rs` still holds `EXTENSION_INSTALL_URL` and opens it once per home until
  j15. The page assets, the Settings kit (`ui/kit/kit.css`) and the loopback routes exist as the
  spec describes.
  (5) `docs/design/adapter-protocol-4.md` also gets one sentence on its store-fallback row,
  answered by JQ2.
- j02: ran the gate checks the files require: (3) `npm test` in `extension/` (464 pass), (5) the
  packager to `.tmp/lane-journey/extension.zip`, (8) `node --test tests/guards/*.test.mjs`, (9) a
  PowerShell parse of the packager, (10) the repository integrity script. The packager check was
  proven once by pointing `setup.html` at `missing.css`: it threw "Packaged page setup.html
  references missing.css, which the package does not include." The page was restored and the
  probe never committed.
  The tree and the spec differed in these places.
  (1) Links use `.gl-page :where(a)` and `.gl-page :where(a):hover`, not `.gl-page a`. A plain
  prefix would out-rank `.gl-btn--primary` and turn a link-button's text the accent colour. The
  focus ring is `.gl-page :where(:focus-visible)` for the same reason: at class weight, later
  component rules such as `.gl-btn` and `.gl-copy` keep their own corner radius when focused.
  (2) The companion has no body wrapper. The step names (`gl-stepper__names`) follow the stepper as
  a sibling and carry the old body padding as `padding-top`. The names use flat element classes,
  `gl-stepper__t` and `gl-stepper__w`. Segments are `gl-stepper__seg--1` and `--2`; segment N joins
  disc N-1 and disc N, and the discs are `--0`, `--1`, `--2`.
  (3) `is-celebrating` goes on `.gl-companion` (as the mockup did on its card). The kit's
  `.gl-mascot.is-celebrating`, `.is-asleep` and `.gl-mascot--sm` are not carried over: the companion
  and `data-light` replace them.
  (4) `.gl-pop` carries `width: 284px` (the mockup got it from viewer chrome). Popup, window and
  page share one `--wash` rule: `[data-light]` waiting .22, off 0.
  (5) The reduced-motion block drops the ember and notch resting rules. With animation off the
  ember already rests on its `--es` and `--eo` values, and the notch no longer exists. The halo
  resting look (.7) stays, and the waiting and off states still dim it.
  (6) `.gl-hr`, `.gl-wrap`, `.gl-stack` and the typography classes are kept as written; no
  extension page uses them yet, and j04 to j07 may drop what they leave unused.
  (7) The kit has no `--r-5` radius token: only the hero card used it.
- Release clock (coordinator, 2026-10-16): the owner's week of daily use starts 2026-10-16, now
  that the shell is complete. Only a change to the service or shell lifecycle (start, stop,
  presence, session end, update) restarts it. Install-time work does not.
- The owner pushed `dev` on 2026-10-16 (832d1abe..8a21f8d1).
- j03: ran the gate checks the files require: (1) `cargo fmt --all -- --check`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo test -p ghostlight-service` (1718 pass), (3)
  `npm test` in `extension/` (466 pass), (5) the packager to `.tmp/lane-journey/extension.zip`, (8)
  `node --test tests/guards/*.test.mjs`, (10) the repository integrity script. `git tag --list 'v2*'`
  was empty. The tree and the spec differed in these places.
  (1) `ServiceBrowserEvents` already held a `WorkbenchFacade` clone, so no new field was needed;
  `reveal` reads the presentation when called, so a late attach works.
  (2) The worker refuses on `!nativePort || !liveState.connected || !liveState.compatible`, as the
  spec says; `runtime_control` keeps its older `!nativePort` check unchanged.
  (3) The Rust round trip of the new fixture is covered by `every_event_decodes_and_round_trips`,
  which walks every fixture and compares the tags with the vocabulary; no per-event test was added.
- j04: ran the gate checks the files require: (3) `npm test` in `extension/` (481 pass), (5) the
  packager to `.tmp/lane-journey/extension.zip` (its page check passed with `setup.js`,
  `lib/setup-view.js`, `kit.css` and the mascot named by `setup.html`), (8) `node --test
  tests/guards/*.test.mjs`, (9) `node --check extension/setup.js` and a PowerShell parse of the
  packager, (10) the repository integrity script. `git tag --list 'v2*'` was empty. The tree and the
  spec differed in these places.
  (1) The unsupported state puts `Not supported` on step 01, as the slice spec says; the mockup put
  it on 02. All three discs are `off`.
  (2) `view()` also returns `fallback` ({ title, text, command }) for the computer and update
  states, and null elsewhere, so the page script holds no words. `lead` is null in those states.
  (3) `kit.css` gains one rule, `.gl-setup__doc`, which pushes the quiet Documentation link to the
  right of the top bar. The service copy of the kit (`ui/kit/kit.css`) is a different file and is
  not touched.
  (4) The link classification comes from `lib/shared.js` (`LINK_HOST_ABSENT`, `LINK_SERVICE_OUTDATED`);
  `lib/setup-view.js` reads it, so `setup.html` loads `shared.js` first. A connected but
  incompatible link that is not outdated shows the start words.
  (5) The meeting plays only when the page sees the change from not met to met while open and
  `ghostlight.setup_met` is not true. A page that opens already connected renders at rest and does
  not set the flag. The fallback, once shown, stays until Ghostlight answers.
  (6) JQ4 is the short sylin.org one-liners from the owner-delegated decisions, not the raw GitHub
  or release-attached forms in the slice spec. `Open Ghostlight` ignores a refusal from the worker
  (the link is down), because the page already shows the waiting words.
  (7) The step names and the Documentation link text are placed by `setup.js` from `WORDS` and
  `LINKS`, not written in `setup.html`, so each string has one home.
- j06: ran the gate checks the files require: (3) `npm test` in `extension/` (494 pass), (5) the
  packager to `.tmp/lane-journey/extension.zip` (its page check passed with `popup.html` naming
  `kit.css`, `lib/shared.js`, `lib/setup-view.js`, `lib/popup-view.js` and `popup.js`), (8) `node
  --test tests/guards/*.test.mjs`, (9) `node --check extension/popup.js`, (10) the repository
  integrity script. `git tag --list 'v2*'` was empty. The tree and the spec differed in these places.
  (1) `popup.html` loads `lib/shared.js` and `lib/setup-view.js` before `lib/popup-view.js`, and no
  longer loads `lib/state.js`, which the popup never used. The sentences "Open Ghostlight on this
  computer." and "It is in the Start menu." are read from `setup-view.js`, so each has one home.
  (2) The hold and resume actions carry the shortcut as a `key` field, absent when the browser has no
  shortcut. The debugger release is `{ label, release: true }`; the script sends
  `release_debugger_sessions`.
  (3) A link that is not connected shows its not-connected words whatever the control state is, so a
  stale `ended` never hides "Not connected". A connected link that is not compatible reads as "isn't
  answering" with the setup button, per the spec; the setup page shows the start words for that case
  (j04 note 4). Reconcile the two if the live check shows it confuses.
  (4) `connection()` for a connected link returns "Connected" and "Ghostlight on this computer is
  answering." (the spec left this open). It ignores the control state; the popup tells that. For an
  unreachable link its line joins the sentence and the place: "Open Ghostlight on this computer. It is
  in the Start menu."
  (5) The popup redraws only when the view changes, so the 1500 ms poll never drops keyboard focus. A
  failed request shows its message in a `role="alert"` line under the actions.
  (6) `kit.css` also gains `.gl-popup` (body width 284 px), `.gl-pop__note`, `.gl-pop__quiet` and
  `.gl-pop__problem`; the section 15 markup comment shows them. The held and recording badge colours and
  the badge text colour are named constants in `lib/state.js`; `BADGE_TEXT_COLOR` is exported.
  (7) `shared.test.js` pinned the old held badge colour and now expects `#5eead4`. The worker test
  fake gains `setBadgeTextColor`, and the unseen-notice test checks the text colour.
  (8) `onboarding.test.js` keeps its linkState tests and the options pins that j07 deletes; its outdated
  test now pins only the options card.
  (9) Review fix: the popup buttons carry `data-intent` (the request each sends). The governance
  acceptance journey (`tests/linux/acceptance/governance.mjs`) now drives hold, resume, end_session and
  start_session by that hook and waits on `#pop[data-light=lit]` and `#title` (Ready, Paused, Session
  ended). `popupText()` in `tests/installed-journey.mjs` waits for the lit light, not a sentence. Both
  are `node --check` only; the coordinator's live runs are the proof.
- j07: ran the gate checks the files require: (3) `npm test` in `extension/` (492 pass), (5) the
  packager to `.tmp/lane-journey/extension.zip` (its page check passed with `options.html` naming
  `kit.css`, `options.css`, `lib/shared.js`, `lib/setup-view.js`, `lib/popup-view.js` and
  `options.js`), (8) `node --test tests/guards/*.test.mjs`, (9) `node --check extension/options.js`
  and a PowerShell parse of the packager, (10) the repository integrity script. `git tag --list
  'v2*'` was empty. The tree and the spec differed in these places.
  (1) `options.html` no longer loads `lib/state.js`, which `options.js` never used; it loads
  `lib/shared.js`, `lib/setup-view.js` and `lib/popup-view.js` in that order.
  (2) The connection pill lost its dot: `gl-pill` has none, and the pill text carries the state.
  The checking state is `gl-pill--quiet`, and the file-opened state is `gl-pill--warn`.
  (3) `connection()` ignores the control state (j06 note 4), so the card shows Connected for a held
  or ended session; the old "Session ended" card is gone. Two cases in `popup-view.test.js` pin
  that and the outdated button. The platform comes from `chrome.runtime.getPlatformInfo()`, read
  once on the first load.
  (4) `shared.test.js` also pinned the options page ("You can always close tabs yourself."); that
  test is deleted with the three in `onboarding.test.js`, which keeps its `linkState` tests.
  (5) The `.visually-hidden` class became the kit's `gl-sr`. `options.css` has no reduced-motion
  block: the kit's `.gl-page` rule already covers every element. The still-historical mention of
  `ui.css` in `docs/tasks/1.0-plus/D1-presentation-stylesheet-module.md` is left as written.
- c01: ran the gate checks the files require: (7) `npm test --prefix packaging/npm` (26 pass) and
  `node --test packaging/mcpb/test/launcher.test.js` (11 pass), (8) `node --test
  tests/guards/*.test.mjs`, (10) the repository integrity script. No Rust, extension or `.ps1`
  file changed, so checks (1), (3) to (6) and (9) did not apply. `winget validate --manifest` on the
  template filled with 2.0.0 and a dummy hash succeeded. `.github/workflows/release.yml` was reviewed
  by eye, not run. The tree and the spec differed in these places.
  (1) The spec describes the pre-edge carriers. Commits e0d24ba1, f7c6ce4d and 42c45a93 had already
  moved them: `EXECUTABLES` is the three images, no arguments run `bin/ghostlight mcp`, the install
  runs `<stage>/ghostlight-service deployment install` with stdout discarded, the development marker
  and same-or-newer checks are in place, `launch.js` runs the installed `ghostlight mcp` after its
  silent install, `package-mcpb.ps1` carries the three images, the WinGet archive script holds the
  three `.exe`, `release.yml` stages the three images per target, and `prepare-npm-package.ps1`,
  `assemble-release-candidate.ps1`, `check-release-candidate.ps1` and `verify-custody.ps1` name six
  raw binaries. None of that needed a change. The launchers' currency check also requires the
  installed service to exist, not the edge alone, which is stricter than the spec and kept.
  (2) The one behaviour change in npm is the never-install branch: it stages `[SERVICE]` and runs the
  staged service directly, as the spec says, where it had staged the edge and service and run the
  edge. The tests that pinned the two-image stage now pin one.
  (3) JQ4 is the short sylin.org one-liners redirecting to `main` (decision of 2026-10-16), not
  release-attached, so `release.yml` does not attach `get.ps1` and `get.sh`. The JQ4 row of the table
  above still says the release attaches them; the decision text supersedes that row, and c02 should
  read it that way. `RESTORE_ROUTE` in both launchers keeps the raw.githubusercontent URLs because
  `language/deployment.rs` words them that way and its test asserts the same text.
  (4) The pre-edge names (`ghostlight-mcp-connector`, `ghostlight-browser-connector`) are gone from
  `tests/fixed-installation-journey.mjs`, `tests/installer-powershell.ps1`, `tests/installer-shell.mjs`
  and `tests/winget-archive.ps1`; none was edited. They remain, outside this slice, in
  `.github/release-template.md`, `llms-install.md`, `SECURITY.md`, `extension/README.md`
  and CHANGELOG history; j17 owns the docs.
  (5) The MCPB README and manifest needed no edit: they carry no connector names. The WinGet
  `InstallationNotes` are c02's and are unchanged.
- c02: ran the gate checks the files require: (9) `sh -n scripts/get.sh` and a parse-only check of
  `scripts/get.ps1`, (10) the repository integrity script. No Rust, extension, packaging code or
  test file changed, so checks (1) to (8) did not apply (the WinGet change is a template text
  line; `winget validate` was not rerun). The scripts were never run. The tree and the spec
  differed in these places.
  (1) The scripts already named the three images (c01 note 1), so the spec's connector-name
  problem did not exist; only the output and the process handling changed.
  (2) The spec asks for a message that keeps today's wording plus the failing command's own
  output. `get.ps1` keeps both messages. `get.sh` had no failure messages (`set -e` exited
  silently), so it gains `ghostlight: installation failed` and `ghostlight: installation did not
  complete. Run '<path>/ghostlight doctor' for details.`, both on stderr.
  (3) JQ4 is main raw: `sylin.org/ghostlight/get.ps1` and `get.sh` redirect to the files on
  `main`. `main` must therefore carry these scripts when v2.0.0 becomes the latest release
  (ADR-0200 Decision 16). Until then the one-liners fetch the 1.x scripts from `main`.
  (4) The `-Restore` and `--restore` header comments and the `RESTORE_ROUTE` text in
  `language/deployment.rs` keep the raw.githubusercontent URLs, as c01 note 3 says.
  (5) Assertions the change breaks for the test-rewrite lane (the tests were read, not edited):
  none fails. `tests/installer-shell.mjs` asserts the request list, the installed files and modes,
  the command order `service: deployment install`, `edge: install` and that the edge's output
  reaches stdout; all still hold, and no assertion reads the old `installed at`, per-file
  checksum or trailing doctor lines. `tests/installer-powershell.ps1` sets `GHOSTLIGHT_NO_REGISTER`,
  replaces `Invoke-RestMethod`, `Invoke-WebRequest` and `Get-Command` (no `gh`), runs the script
  twice, and asserts the installed bytes, the `Checksum verification failed` message, the
  preserved installation, the removed temporary directories and the installed file set; all still
  hold. Stale wording only: the PowerShell test's PASS line says "waited native install" and its
  header says the console output of `ghostlight install` is an owed live check. The fixtures do not
  assert the new verify line, the `GHOSTLIGHT_NO_REGISTER` sentence, the provenance variant or the
  failure messages; add them.

- x01: ran the gate checks the files require: (3) `npm test` in `extension/` (492 pass), (5) the
  packager to `.tmp/lane-journey/extension-2.0.0.zip` (twice, same hash), (8) `node --test
  tests/guards/*.test.mjs`, (9) `node --check` on the two new modules and a PowerShell parse of the
  packager, (10) the repository integrity script, `cargo test -p ghostlight-service --test
  store_listing_copy` (the paste blocks fit 1000 characters; no other Rust changed, so fmt, clippy
  and the full service tests did not run), and `scripts/check-public-surfaces.ps1` offline (it
  passes; its warning about a missing compatibility row for service 1.3.15 is older). The freeze
  guard was proven once: a space appended to `extension/kit.css` failed it naming `kit.css` and
  ADR-0200 Decision 16; the file was restored and the probe never committed.
  The freeze. `git tag --list 'v2*'` was empty. The recording fix (c33bf24f, ledger row
  ci-recording-source-evidence in seams.md) had landed. The zip: 47 files, 199979 bytes, version
  2.0.0, `key` stripped, SHA-256 5bd88867bcb2640a8101e518f3ab7c7f3842b794dd566c694631fa51617c053e
  (built with PowerShell 7.6.5; the manifest's JSON layout comes from `ConvertTo-Json`, so
  Windows PowerShell 5.1 would produce a different manifest and zip hash). After this slice a
  packaged file changes only with a recorded owner decision to withdraw and resubmit, and the
  golden is regenerated with `node tests/guards/lib/extension-package.mjs <zip>` in the same
  commit.
  The tree and the spec differed in these places.
  (1) Preconditions. The ledger recorded none of these: the c02 live check of the exact
  one-liners, the tuned fallback constants (`FALLBACK_AFTER_COPY_MS` and `FALLBACK_AFTER_OPEN_MS`
  are still 90000 and 120000), the approval of j07's strings, the sylin.org redirects answering,
  or the JQ3 live check. The coordinator's task for this slice carried the binding decisions of
  2026-10-16 and the policy reading, and asked for the slice, so it ran. It prepares only. The
  constants and the strings are in the frozen package: tuning either later is a packaged-file
  change.
  (2) The spec's remote-code answer ("Yes", with the two sentences) is superseded by the owner
  ruling: the answer is No. The two sentences are in the justifications and the disclosure draft
  as the plain statement of what the page code is and where `browser_execute` text comes from.
  (3) JQ3: host permissions stay, because no live-check result is recorded. The justification
  block no longer speaks of content scripts; it says the extension has none, uses no scripting
  API, and works through the Debugger API in controlled tabs.
  (4) The `downloads` block gained the options page's "Save developer diagnostics" download. The
  recording save is an agent request under local policy, not a person's click, and the block now
  says so. The `offscreen` claim checked out against `offscreen.html` (it loads only packaged
  scripts and makes no request).
  (5) The golden hashes text files with LF line endings and the manifest after the `key` strip in
  the packager's serialization (CRLF lines, one trailing LF). Working copies on this machine hold
  CRLF in some files that git stores as LF, so the packager was changed to stage text files with
  LF; without that the zip hash depended on the checkout. The guard reads the packaged lists from
  the packager script and takes `lib/` and `vendor/` whole, so a new root file or a new `lib` file
  fails it.
  (6) The guard runs in CI in the extension job after the store artifact step; `AGENTS.md` names
  it in the building section. `docs/public-status.json` already says the injector loads the
  installed service runtime, which agrees with the new wording; it was not changed.
  (7) `ACCEPTANCE.md` gate 3 also carries the setup-page sentence (the first connection that finds
  the service absent opens the bundled page once), and gate 4 drops the "returns revision and hash
  through a correlated receipt" and "retires the connection" claims, which the protocol-4 code and
  README do not state.
- x02: ran the gate checks the files require: (9) `sh -n scripts/get.sh`, a parse-only check of
  both PowerShell files, and `node --check tests/installer-shell.mjs`; (10) the repository integrity
  script. `tests/installer-shell.mjs` is Linux-only and `tests/installer-powershell.ps1` starts the
  fixture service, so neither ran here. The requested-version logic was exercised in scratch
  copies instead: `get.sh` against a stub `curl` (an existing prerelease proceeds to the checksum
  fetch, an absent tag and a reply for another tag print "release v<version> does not exist", a
  malformed version is refused) and `get.ps1` against stub web calls in Windows PowerShell 5.1
  and PowerShell 7 (a 404 answers the one sentence, other failures rethrow, a prerelease proceeds).
  No script ran against GitHub. The coordinator runs the two test files on Linux and Windows.
  The tree and the spec differed in these places.
  (1) The shell script finds a requested tag through `api.github.com/repos/<repo>/releases/tags/<tag>`
  and reads the HTTP status itself (`curl -w` with the http_code format, no `-f`): 200 proceeds, 404
  is "does not exist", and any other status or a failed connection says it could not look the
  release up. (The first version read curl's exit 22 as "does not exist", which a 403 rate limit
  also produces; review fixed it.) The PowerShell script treats a 404 the same way and rethrows
  other errors.
  (2) A requested version must look like `2.0.0` or `2.0.0-rc.1`; the latest path still insists on a
  stable three-part tag.
  (3) The earlier fork wording in the JQ4 row and the c02 owed check is replaced; ADR-0200
  Decision 6 and the amendment "the cutover is one step" are left as written and superseded only
  where they differ, by the new amendment.
- x02 review fix: two blocking review findings. (1) `get.sh` no longer reads curl's exit 22 as an
  absent release: the tag lookup drops `-f`, reads the HTTP status and refuses only a 404 as "does
  not exist"; a 403, a 5xx and a failed connection say the lookup failed. The curl stub in
  `tests/installer-shell.mjs` now answers a status the way real curl does (exit 0 with the code),
  with new 403, 503 and no-connection cases; `tests/installer-powershell.ps1` gains a 403 case.
  (2) Step 2 of the release order had no command. `publish-github-release.ps1` gains
  `-Prerelease <suffix>`; step 2 now has the three exact commands, and the document, the ADR-0200
  amendment and the certification notes say the rc carries the 2.0.0 build. Gate: (9) `sh -n`,
  parse-only checks of the three PowerShell files, `node --check`; (10) the integrity script.
  `tests/installer-shell.mjs` is Linux-only, so a scratch copy ran under Git Bash with a `uname`
  stub and the file-mode assertion removed, and passed; the `get.ps1` 403 and 404 paths ran
  against a stub in PowerShell 7. The publish script was parsed, not run. The coordinator runs the
  two installer tests on Linux and Windows.
- j09: ran the gate checks the files require: (1) `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test -p ghostlight-service`, and (10) the repository integrity
  script. No extension, page-asset, packaging or guard file changed. The frozen-set precondition held
  (`git tag --list 'v2*'` is empty).
  The tree and the spec differed in these places.
  (1) The spec asks that a status be rendered "the way work/receipt.rs does"; that code was inline,
  so it became `work::result::Status::stored_name()` and the receipt uses it too.
  (2) `audit::unix_ms` became `pub(crate)` for the setup record's migration time instead of a third
  copy of the clock helper.
  (3) The break migration's `MigrationPlan` gains a `setup_record` field (the path to write, set only
  when a 1.x installation, its marker, a move or an audit archive shows the home was in use, and
  no record exists), written by `copy_state`; the serialized plan test lists the new key. A plan
  with only earlier 2.0 development leftovers does not write one.
  (4) `handoff.rs` is untouched: it still reserves `extension-handoff-v1` in the state directory for
  a fresh install until j15, and a migrated home no longer carries the 1.x marker.
  (5) `WorkbenchProjection` holds the progress; the facade's `attach_setup` and `setup()` reach it
  there, so the service and j12 share one object.
- j10: ran the gate checks the files require: (1) `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test -p ghostlight-service`, and (10) the repository integrity
  script. No extension, page-asset, packaging or guard file changed.
  The tree and the spec differed in these places.
  (1) `scan(registry)` returns `Vec<BrowserProfiles { browser: BrowserIdentity, scan: ProfileScan }>`;
  the spec named no return type. `ProfileScan.profiles` is a count, `with_extension` the list.
  (2) A key is also refused when it is not a single normal path component (`.`, a drive prefix),
  beyond the list in the spec, so no key can name the user data directory itself.
  (3) A `Local State` that parses but lists no usable profile (empty `info_cache`, or every key
  hostile) falls back like an unparsable one, so a hostile file cannot make the count zero.
  (4) `info_cache` is read with a small ordered-map visitor because `serde_json` here does not keep
  key order; the browser's order is the order of `with_extension`.
  (5) The two Rust files were formatted with `rustfmt` on those files, not `cargo fmt -p`, to leave
  other lanes' files alone.
- j12: ran the gate checks the files require: (1) `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test -p ghostlight-service` (1813 pass), (6) the workbench
  fixtures (`node tests/workbench-surface.mjs`, `node --test tests/workbench-transport.test.mjs
  tests/workbench-settings.test.mjs`), (9) `node --check` on the two fixture servers, and (10) the
  integrity script. No extension, page asset, golden, guard or `crates/orchestrator/ui` file changed,
  so (2) to (5), (7) and (8) did not apply and the extension freeze held.
  The tree and the spec differed in these places.
  (1) Screens are told with a hint, not the whole part: `part_changed` with `part` `setup` and the
  tag, like the integrations and the policy. A hint cannot leak the part to a watcher (whose hint
  carries no tag) and keeps one projection path; the window reads `parts=setup` on a hint. The page
  ignores a hinted part it does not know, so the current window is unaffected until j14.
  (2) `service/mod.rs` does not wire `on_complete`: `WorkbenchFacade::attach_setup` does, with a
  weak reference to the live parts, so the service's one `attach_setup` call and every test that
  attaches progress through the facade get it. The only change in `service/mod.rs` is that the
  readiness control operation calls the new `WorkbenchFacade::readiness()` instead of building a
  whole snapshot, so a readiness check does not read the machine while setup is incomplete.
  (3) Every publication of the integrations part rebuilds setup, not only `refresh_harnesses`:
  a client act (install, fix, uninstall, locate, set up all) changes the agent rows too, and j13
  needs the part to follow them.
  (4) A service with no `SetupProgress` attached tracks no setup: the part is the retired view and
  nothing is built or announced. This keeps every test that does not care about setup unchanged.
  (5) `agent_name` is resolved once, when the finishing session is still live, and kept
  (`Remembered`), so ending that session does not change the Done headline. It also accepts a
  label equal to a roster label, and a label equal to a product id is named by the product name
  (the roster label of one of several hosts would be wrong).
  (6) Unknown connected browsers are listed as `other` with the reported name and the short name
  `Chromium`. The setup part is not in `WorkbenchSnapshot`: `WorkbenchFacade::setup_view()` builds it,
  and the snapshot route calls it only for a request that names `setup` (or names no parts) from an
  audience that is shown the part (`Projection::part` takes the builder and calls it only then). So a
  watcher's snapshot, and a window snapshot that leaves `setup` out (the six parts the window asks
  for today), read nothing from the machine. A review found the first version built it for every
  snapshot, which let any loopback caller repeat the reads without the `RESCAN_SPAN` limit.
  (7) The fixtures add a `retired` setup part to `tests/workbench-preview-server.mjs` and
  `tests/workbench-surface.mjs` (control view only). `SNAPSHOT_PARTS` in the window's `transport.js`
  still names six parts; j14 adds `setup`, and `workbench-transport.test.mjs` is unchanged until then.
  (8) `Live::new` reads the start time itself instead of taking it, to keep its argument count under
  the lint.
- j13: ran the gate checks the files require: (1) `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test -p ghostlight-service` (1827 pass), (6) the workbench
  fixtures (`node tests/workbench-surface.mjs`, `node --test tests/workbench-transport.test.mjs
  tests/workbench-settings.test.mjs`), (9) `node --check tests/workbench-preview-server.mjs`, and (10)
  the integrity script. No extension, page asset, golden, guard, relay, edge or manifest file changed,
  so (2) to (5), (7), (8) and the workspace test run did not apply, and the extension freeze held.
  The tree and the spec differed in these places.
  (1) The spec says to launch "with the graphical-session environment `launch_unless_ending` uses".
  `os::spawn_detached` takes no environment, and the relay source is locked to the published relay
  revision (the connectors guard), so it could not be extended here. `ThisComputer::start` proves the
  session with `graphical_session_environment()` and refuses without one, and the browser inherits
  this process's environment, which holds the display that proof found. What is not applied on Linux
  is the overlay of `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` defaults for a service started
  with neither; the live check above covers a thin environment. `recovery.rs` is untouched.
  (2) The working directory of the start is the user's home (`NativeHostRegistry::user_home`, the
  `USERPROFILE` or `HOME` the registry already uses; the temporary directory when it names none).
  `spawn_detached` needs a directory, and the Ghostlight home or the browser's own directory would be
  held open by a long-lived browser.
  (3) `SetupMachine::browser_found(id) -> bool` became `browser_executable(id) -> Option<PathBuf>`; the
  view asks `is_some()`. Edge is allowed to Add (JQ9 = yes), with the same listing URL.
  (4) A step of kind `get_chrome`, `connect_agent` or `review_agent` posted to `/api/setup/act` is
  refused with the "no longer offered" sentence (422), not a bad request: the body is the shared
  `SetupAction`, and the service performs only the two browser kinds.
  (5) The spec asks for the harness install check "through `/api/integrations/act`". The test drives
  `WorkbenchFacade::manage_harness_expecting`, which that route calls, over an isolated registry with
  the edge in place and a Cursor configuration found; the route itself is tested in `web/tests.rs`.
  It checks the setup part is announced again and reads "Restart Cursor to finish.". The private-view
  refusal is unchanged and still tested with the roster.
  (6) The act key reaches the page: `ACT.setupAct` in `ui/lib/transport.js` and the act lists in
  `tests/workbench-transport.test.mjs`, `tests/workbench-surface.mjs` and
  `tests/workbench-preview-server.mjs`, because the transport test requires the page to name every
  act the context lists. The page has no `ROUTES` command for it yet; j14 adds it.
  (7) The About-page destination test skips `GetChrome`: it is Google's address, reached from the setup
  view and not from the About page, and the test requires the product's own hosts. No `words.js`
  change.
- j14: ran the gate checks the files require: (1) `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test -p ghostlight-service` (1829 pass), (6) `node tests/workbench-surface.mjs`
  and `node --test tests/workbench-transport.test.mjs tests/workbench-settings.test.mjs`, (9) `node --check` on
  `app.js`, `lib/setup-view.js`, `lib/transport.js`, `lib/view.js`, `lib/words.js` and the three `tests/*.mjs`
  fixtures, and (10) the integrity script. No extension, page asset, golden, guard, relay, edge or manifest file
  changed (the extension kit is embedded, not edited), so (3), (4), (5), (7), (8) and the workspace test run did
  not apply, and the extension freeze held. The tree and the spec differed in these places.
  (1) The setup part is read on its own (`transport.setupPart()`, `GET /api/snapshot?parts=setup`) and is not
  added to `SNAPSHOT_PARTS`. The j12 note said j14 would add it there, but an unfinished setup reads this computer
  and every resync would pay for that; the six-part table and its test are unchanged. The latest read asked for
  wins, and the answer of a setup step is taken the same way.
  (2) The meeting's done row cannot say a bare "Connected": the agent view holds no such word. The row shows
  the second step's word ("Connected in Chrome"), and the row is the one whose last name word starts the
  connected line, or else the one the person clicked.
  (3) "After 6000 ms more" is counted from the moment the completion is drawn, not from the tagline.
  (4) With motion, the tagline arrives through the kit's own delay (1050 ms, `is-new`); with reduced motion a timer
  shows it at 1050 ms. The segment sweep at 150 ms, the ring at 120 ms and the motes from 200 ms are the kit's
  CSS, not timers.
  (5) There is no `setup-note` element (j11 note 2). The markup adds `app-shell` (the id the page hides) and
  `setup-agents` (the block around the rows label and list), and the code panels carry no `$` glyph because
  `CodePanel` has no prompt flag.
  (6) The window's motion preference is the system setting or the Settings Motion choice (`motion-reduced` on
  the body). The command palette (Ctrl+K) does not open over setup.
  (7) `WorkbenchFacade::show_address()` (`workbench/mod.rs`) is new: a browser's show request reveals at it. The
  install's own show (the control operation `show`) still reveals with no address; that is j15's.
  (8) The tray's Open sends no address, because the shell decides it. While setup is incomplete the service
  now follows the click with a show at `#setup` (the `TrayItemId::Open` arm of the shell intake loop reveals at
  `show_address()` on a short-lived thread, as a browser's show request does), so a window that skipped and is
  still open, behind others or minimized, comes back at setup. A window opened fresh loads setup by itself. After
  completion `show_address()` is `None` and the tray changes nothing. The reveal shares a one-in-flight guard, so
  repeated clicks do not stack. On Linux the tray's Open already builds a fresh window (Wayland cannot raise one),
  and that page needs nothing more, so the shell counts the follow-up show as the answer to the click: the
  lifecycle remembers a tray Open that built or began replacing the window (`open_from_tray`) and the next show
  within three seconds opens nothing again (`answers_tray_open`). A pending replacement still takes the address.
  On Windows the click only focuses, so the show still focuses and navigates to `#setup`.
  (9) `lib/view.js` exports `writeAddress`, so the page writes `#setup` in place when the view opens.
  (10) `tests/workbench-settings.test.mjs` expected the address list to be derived from the tabs; it now expects
  the tabs and then the setup address. The preview server's module pattern allowed no hyphen, so
  `lib/setup-view.js` was refused; it now allows one and serves the two extension files under `/extension/`.
- j15: ran the gate checks the files require: (1) `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test -p ghostlight-service` (1836 pass), (7) `npm test --prefix
  packaging/npm` and `node --test packaging/mcpb/test/launcher.test.js` (the Linux completions changed), (9) `sh -n`
  on the bash completion (unchanged) and no runtime JavaScript, and (10) the integrity script. No extension, page
  asset, golden, guard, relay, edge or manifest file changed, so (3), (4), (5), (6), (8) and the workspace test
  run did not apply. The tree and the spec differed in these places.
  (1) The spec asked the install to call the window "at `PageAddress::Setup`". The CLI sends no address: the
  control operation `show` asked for none, so the service now reveals at `show_address()` (`service/mod.rs`,
  `serve_control`), which is `Setup` exactly while setup is incomplete, the same call a browser's show request and
  the tray's Open make. `ghostlight open` therefore also opens at setup while setup is incomplete. A test pins
  both halves (`a_show_control_opens_the_window_at_setup_until_setup_is_complete`).
  (2) `ghostlight doctor --fix` runs the install steps and would now show the window on an incomplete setup, which
  a repair should not do, so it passes `no_open: true`. It still prints the summary.
  (3) A real install reads the client roster even with `--no-clients` (a read, nothing written), so the summary
  can name the agents that are here and give the MCP command. The "needs attention" lines for clients print only
  when clients are in scope.
  (4) The browser facts for the summary come from the same reads the setup view makes. Those reads moved out of
  `setup_view` into `browsers_here` (`workbench/setup.rs`) and the install calls `browsers_on_this_computer()`;
  no connected browser is known to a command-line process, so `connected` is false and the Next line is decided
  by the profile scan alone.
  (5) A usable browser registration that is not usable for the install (`install_usable` false) still ends in the
  error it ended in before, and now prints a `Browser:` line for every browser the install chose so the reason is
  on the screen; with a usable install only a registration that needs attention or is owned elsewhere prints.
  (6) `language::workbench::stray_stopped_line` had no other caller and was deleted with its assertion. The stray
  that would not stop is still said, and a stray that was stopped is not.
  (7) Uninstall output is unchanged. The source file was checked out with CRLF line endings in this working tree;
  it is written with LF, as the repository stores it.
  (8) `docs/1.0/ARCHITECTURE.md` still says "the extension handoff marker" and "the install handoff opens no
  page"; those and the trust pages belong to j17.
