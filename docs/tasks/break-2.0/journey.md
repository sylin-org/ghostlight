# Break 2.0: install journey ledger

The journey lane carries [ADR-0200](../../adr/0200-the-install-journey.md): one install journey in
three steps (This computer, Your browser, Your agent) across the extension's setup page, the popup,
the window's setup view and the install output. It follows the seams ledger ([seams.md](seams.md))
and the shell ledger ([shell-m2.md](shell-m2.md)); ADR-0199 wins over ADR-0197 where they differ and
ADR-0200 wins for the journey. The wire is [adapter-protocol-4](../../design/adapter-protocol-4.md).
The ground rules are the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md) plus the gate and rules
below. Slices, in order: j01-adr, j02-kit, j03-show-event, j04-setup-page, j06-popup, j07-options,
j08-toolbar-icon, c01-carriers, c02-get-scripts, x01-store-submission, j09-first-request,
j10-profiles, j11-setup-language, j11b-agent-outcomes, j12-setup-part, j13-setup-actions,
j14-window-view, j15-install-cli, j17-docs. A slice whose owner answer is Pending stops and says so
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
| JQ4 frozen URLs and the pre-submission test release | `irm https://sylin.org/ghostlight/get.ps1 \| iex`, `curl -fsSL https://sylin.org/ghostlight/get.sh \| sh`, documentation link https://sylin.org/ghostlight/. Both redirect to the latest GitHub release assets; the release attaches `get.ps1` and `get.sh` (c02). The website redirect is the owner's to publish; x01 waits until both answer. The test release is made in a fork. | 2026-10-16 |
| JQ5 a Watch tab during setup | Shows the ordinary workbench. | 2026-10-16 |
| JQ6 installed but not running | "Open Ghostlight on this computer." and where it is (Start menu or Applications), on the setup page and in the popup. | 2026-10-16 |
| JQ7 scope of 2.0.0 and the cutover | 2.0.0 waits for the service half (j09 to j15). | 2026-10-16 |
| JQ8 Add and Open <browser> | Ghostlight starts that browser. | 2026-10-16 |
| JQ9 Edge | Add from the Chrome Web Store until an Edge listing exists. | 2026-10-16 |
| JQ10 window on the first no-browser refusal | None. The agent's answer carries the link. | 2026-10-16 |
| JQ11 Skip for now | Kept, quiet; the tray brings setup back until done. | 2026-10-16 |

## RESUME HERE

x01-store-submission has landed (it prepares only; nothing was submitted). Next: j09-first-request. Rule 8 now binds: a packaged extension file changes only with a recorded owner decision to withdraw and resubmit, and the freeze golden is regenerated in the same commit.

## Landed

| Slice | What | Commit |
|---|---|---|
| j01-adr | ADR-0200 is filed and indexed with Decisions 1 to 16 and the owner answers above. The ADR index marks ADR-0070, 0091, 0095, 0126 and 0177 as amended by it. The adapter-protocol-4 note gains a marked amendment superseding the section 7 rows on publishing after services spread and the unchanged level-3 path. This ledger starts; `docs/tasks/README.md` and the seams ledger point here. | see `git log` |
| j02-kit | `extension/kit.css` is the one stylesheet for the extension pages: the sylin.org night tokens and base rules under `.gl-page`, the kit components, the setup layout, the companion with its two-segment stepper, and the popup. `scripts/package-extension.ps1` ships `kit.css` and `icons/ghostlight-mascot.png` and throws when a staged page names a local file the package lacks. The source-grep test for `setup.html` is deleted. | see `git log` |
| j03-show-event | Protocol 4 gains one browser-level event, `show_requested`, with no fields: the golden vocabulary and fixture, `extension/lib/protocol.js`, the worker (`show_ghostlight` page message, `requestShow()`, refused while the link is down), `wire::Event::ShowRequested`, `BrowserEvent::ShowRequested`, and `ServiceBrowserEvents` calling `WorkbenchFacade::reveal(None)` and ignoring the answer. The adapter-protocol-4 note carries the row and the browser-level sentence. | see `git log` |
| j04-setup-page | `extension/setup.html` is the kit page: the top bar with the wordmark and the Documentation link, the companion (ghost, lantern, stepper, step names) and the one column of eyebrow, heading, command, live line and fallback. `extension/lib/setup-view.js` (new, pure) holds every word and command and `view()` and `fallbackDue()`; `extension/setup.js` (new) places them, polls `ui_snapshot` every 1500 ms, plays the meeting once (`ghostlight.setup_met`) and sends `show_ghostlight`. The popup and options setup routes always open the bundled page and both remote URL literals are gone. The packager ships `setup.js`. The tests that pinned `setup.html` text are deleted; `extension/tests/setup-view.test.js` covers the pure module. | see `git log` |
| j06-popup | `extension/lib/popup-view.js` (new, pure) holds every popup word, `popupView()` and `connection()`. `popup.html` is the kit page (mascot at 100 px, status block, actions, Settings link) and `popup.js` places the view, redrawing only when it changes. The captions switch, the notes, the setup paragraph, the debugger line and the version are gone. The held badge is the accent `#5eead4`, and `worker.js` sets the badge text colour `#08110f`. `kit.css` gains the popup note, quiet-button and problem rules. The tests that pinned popup text are deleted; `popup-view.test.js` and `state.test.js` are new. | see `git log` |
| j07-options | `extension/options.html` is a kit page (`<html class="gl-page">`, `<body class="gl-stage options-page">`) linking `kit.css` then the new `extension/options.css`, which holds the cards, rows, switches and footer on kit tokens. The connection card's pill and line come from `GhostlightPopupView.connection()`: `gl-pill--ok` when connected, else `gl-pill--warn`; the setup button shows whenever `needsSetup` is true, is labelled `setupLabel` and opens the bundled `setup.html`. The words pass is done (strings under Owed to the owner). `extension/ui.css` is deleted and the packager ships `options.css` instead. The options source-text pins in `onboarding.test.js` and `shared.test.js` are deleted; `popup-view.test.js` gains two connection cases. | see `git log` |
| c01-carriers | The carriers ship and run the three 2.0 images (`ghostlight`, `ghostlight-service`, `ghostlight-desktop`). Most of the slice had landed in earlier carrier fixes (see Notes); this commit finishes it. The npm never-install commands (`doctor`, `status`, `help`, `--version`, `install --help`) with nothing installed now stage and run `ghostlight-service` itself instead of a staged edge beside a staged service, so they download one image, not two. The WinGet template lists `ghostlight-service.exe` and `ghostlight-desktop.exe` as nested installer files with no alias, beside `ghostlight.exe` with the alias `ghostlight`; `winget validate` accepts it. The npm README says `npx -y ghostlight` is the MCP server and `npx -y ghostlight install` installs. | see `git log` |
| c02-get-scripts | `scripts/get.ps1` and `scripts/get.sh` download the three 2.0 images and verify them as before (checksums, and `gh attestation` when `gh` is present). They print one line after verification, `Ghostlight <version> downloaded. Checksums verified.`, or `... Checksums and build provenance verified.` when every file passed attestation. The service's `deployment install` runs with stdout discarded and errors shown, then the installed `ghostlight install` runs in the same console so its summary prints and its exit code is checked. `get.ps1` uses the call operator and `$LASTEXITCODE` for both, with no `Start-Process`. The trailing doctor line is gone; the doctor hint stays inside the two failure messages. `GHOSTLIGHT_NO_REGISTER=1` prints `Ghostlight <version> is installed in <dir>. Run 'ghostlight install' to connect your browsers and agents.` Each header carries the frozen-URL sentence and the short sylin.org one-liner. The WinGet `InstallationNotes` say to run `ghostlight install` once and again after `winget upgrade`. | see `git log` |
| x01-store-submission | The store copy is true to the tree and the package is frozen. `PERMISSION_JUSTIFICATIONS.md` says page code is part of the application installed on this computer, supplied over native messaging and checked by hash, and that `browser_execute` text is written by the person's agent; the debugger, downloads, nativeMessaging, offscreen, storage and host-permission blocks follow the code (no content scripts, no scripting API, the options page's diagnostics download, the setup flag and the session keys). `PRIVACY.md`, `STORE_LISTING.md` (setup-page sentence, assets, submission gate), `trust/faq.md`, `trust/questionnaire.md`, `LICENSING.md`, the `gifenc.js` comment, `extension/README.md` and `ACCEPTANCE.md` extension gates 3, 4 and 7 are corrected. `docs/release/store-disclosure-2.0.md` (new) holds the dashboard answers (remote code: No, per the owner ruling), the dated Chrome and Edge policy reading, JQ2's answer, the staged release order and the assets to recapture. The freeze: `tests/golden/extension-2.0.0.files.sha256` (47 packaged paths and the zip's hash) and `tests/guards/extension-freeze.test.mjs` (with `tests/guards/lib/extension-package.mjs`), run by the extension CI job. The packager now stages text files with LF line endings, so one commit packages to the same bytes on every machine. | see `git log` |

## Owed live checks

- j01: none. Docs only.
- j02: none. The page markup that links the kit lands in j04, j06 and j07; the live look is checked there.
- j03: the unpacked extension's Open Ghostlight brings the window forward on Windows and Linux (live, once j04 lands the button). The button has landed in j04; the check is still owed.
- j04: the unpacked extension in a fresh profile on a machine with no Ghostlight: the page opens once, the command copies (and `Copied` is announced), the meeting plays once when the service from the development channel answers, and Open Ghostlight shows the window. Also reduced motion, a 390 px window, and the Documentation link opening in a new tab.
- j04: measure a cold get-script run on test-01 after c02 and tune `FALLBACK_AFTER_COPY_MS` (90000) and `FALLBACK_AFTER_OPEN_MS` (120000) in `extension/lib/setup-view.js` before x01.
- j06: the popup in each state (Ready, Paused, Session ended, Not connected with the install and the start words, Update needed, recording, debugger release) on Chrome and Edge, on Windows and Linux. The key cap and `aria-keyshortcuts` follow a changed shortcut. The mascot looks right at 100 px in the lit and waiting lights. The badge text is readable on the accent.
- j04: the coordinator's check that the JQ4 one-liners run in Windows PowerShell 5.1, PowerShell 7 and `curl | sh` is not recorded in this ledger yet.
- j07: the options page on Chrome and Edge, on Windows and Linux: it scrolls (the stage no longer clips), every switch still saves and survives a reload, the captions switch dims while effects are off, Save developer diagnostics still downloads, the connection card reads right in each state (Connected, Not connected with and without the setup button, Update needed), and the setup button opens the bundled setup page. The kit look is right at 390 px and with reduced motion.
- c02: the exact JQ4 one-liners, `irm https://sylin.org/ghostlight/get.ps1 | iex` in Windows PowerShell 5.1 and in PowerShell 7, and `curl -fsSL https://sylin.org/ghostlight/get.sh | sh` in a Linux shell, against a 2.0 test release made as JQ4 says (a fork, or a non-prerelease test tag: `releases/latest` skips prereleases and drafts; the fork test uses the fork URLs directly). Each shows one verify line, then the install summary. A failure (a development install present) shows its reason and the failing command's own output. The install summary prints in the console on Windows now that nothing is hidden. A cold-run time on test-01 for j04's fallback constants (`FALLBACK_AFTER_COPY_MS`, `FALLBACK_AFTER_OPEN_MS`). All of these are needed before x01.
- x01: an install of `.tmp/lane-journey/extension-2.0.0.zip` as an unpacked folder on Chrome and Edge. After the owner submits (staged), the staged package itself through trusted testers; a packed Chrome clamps `chrome.alarms` to 30 seconds, so measure the time to connect after an install.
- x01 (JQ3): the unpacked extension with `host_permissions` removed, on an http and an https page, covering attach, `webNavigation` events, capture, navigate, read, click, screenshot, execute and recording. No result is recorded, so the package keeps the host permissions. If nothing needs them, remove the block, the host-permission justification and its listing line, regenerate the golden, and run the check again on the final zip.
- c01: `npx -y ghostlight install` on test-01 (Windows) and on a Linux machine; then an MCP client with `npx -y ghostlight` connects. `npx -y ghostlight --version` on a machine with nothing installed downloads and runs only the service. The WinGet alias `ghostlight` finds `ghostlight-service` beside its target (the portable zip extracts all three to one folder). The Claude Desktop MCPB on Windows. The window and tray appear after each.

## Owed to the owner

- Publish the sylin.org redirects `/ghostlight/get.ps1` and `/ghostlight/get.sh` to the latest
  GitHub release assets (separate repository). x01 waits until both answer.
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
