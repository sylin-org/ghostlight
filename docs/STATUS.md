# STATUS -- Ghostlight 2.0.0 published

This page is the current state and nothing else. Every earlier dated section, from the 1.0 rebuild
through 2026-10-16, moved verbatim to [history/status-through-2026-10-16.md](history/status-through-2026-10-16.md).
Decisions live in [adr/](adr/README.md), standing preferences in [MEMORY.md](MEMORY.md).

## Published

- Release 2.0.0 is the latest public release, published on 2026-10-19: the GitHub release
  `v2.0.0`, npm `ghostlight@2.0.0`, the Chrome Web Store extension 2.0.0 and the MCP Registry
  record `org.sylin/ghostlight 2.0.0` (`docs/public-status.json`, `server.json`,
  `compatibility.json`, and the custody record
  [testing/release-2.0.0-2026-10-19.md](testing/release-2.0.0-2026-10-19.md)). The WinGet
  manifests merged on 2026-10-20 (microsoft/winget-pkgs#456409); that morning WinGet's index still
  listed 1.3.14.
- 1.3.15 was never released. The owner withdrew it on 2026-10-16; the `v1.3.15` tag stays as
  history.

## Branches

Only `main` and `dev`, locally and on the remote (AGENTS.md, Branches). `main` holds the published
2.0.0 product and its public state (e789913b). `dev` holds all work toward the next release.
Older branch heads are kept as `archive/` tags.

## 2.0 work

`dev` carried the 2.0 break set of
[ADR-0197](adr/0197-contained-seams-and-the-2-0-break.md), the one-service-and-thin-shells design of
[ADR-0199](adr/0199-one-service-thin-shells.md) and the agent experience of
[ADR-0198](adr/0198-agent-experience.md). The product version is 2.0.0 everywhere it is stamped.
The ledgers below are the record of what was done; read the RESUME HERE block in each.

- **Seams** ([seams.md](tasks/break-2.0/seams.md)): the removals, policy schema 4, audit v2, relay
  protocol 4, the edge, the carriers, the lean blueprint F1 to F13, and the handles slices.
  Blueprint steps F11 to F13 are complete (the pristine pass, the frozen-code guards, the 2.0
  version and the current-state documents, including this page). Owed live checks and owner items
  are listed in the ledger.
- **Install journey** ([journey.md](tasks/break-2.0/journey.md)): the three-step install, the
  bundled setup page, the window's setup view, the popup and options words, the get scripts and the
  packaged extension. Complete through j17-docs. The setup view offers Add in every found
  Chromium-family browser (Chrome, Brave, Chromium and Edge), because all of them install the
  extension from the Chrome Web Store; no Edge Add-ons listing is requested unless the owner asks
  for one. Owed live checks (fresh homes on Windows and Linux, the install from the release
  candidate's bundle) and the store submission are listed in the ledger.
- **Desktop shell** ([shell-m1.md](tasks/break-2.0/shell-m1.md),
  [shell-m2.md](tasks/break-2.0/shell-m2.md)): the service serves the workbench on 127.0.0.1 and
  `ghostlight-desktop` is a webview on it. Milestones 1 and 2 are complete and deployed on the
  owner's machine: Settings, the tray, the shell's own life and idle exit (a service with no
  stream of any role for 10 minutes exits). Next: the owner's checkpoint 2 (tray icon and state
  dot, first-close hint, Start-menu launch, Quit in Settings, Continue at a section from a watch
  tab).
- **Window design** ([design.md](tasks/break-2.0/design.md)): landed on `dev` on 2026-10-17 and
  awaits the owner's live check. The same ledger records the agent catalog, the icon endpoint and
  its tiles (decisions 9 and 10), a Store app's own settings copy found on every look (t04,
  ADR-0197 amendment of 2026-10-17), and the test hygiene slices t01, t02 and t05. All of it is
  pushed and deployed on the owner's Windows machine; CI was all green on both systems at
  51e1cecb.
- **Linux** ([linux-1](tasks/linux-1/BOOTSTRAP.md)): opened 2026-10-17, closed 2026-10-18. A
  Claude Code session on the owner's CachyOS machine worked the owed Linux checks L01 to L10 on
  the real installation and, by the owner's leave, fixed what it found (each fix is in the
  ledger, the record). On 2026-10-18 its open questions were settled by the user-delight test and
  old leftovers removed (ledger, "Follow-up"). Owed: the install from the release candidate's
  bundle.
  On 2026-10-18 the owner unfroze the extension for a proper fix of saved tab groups piling up, and
  it landed the same day ([ADR-0201](adr/0201-a-kept-tab-leaves-the-group.md), 0018941b): a tab
  preserve-tabs keeps after its session ends leaves Ghostlight's group, so Chromium deletes the
  idle group instead of saving it, and reuse adopts the tab back. `lib/tabs.js` is the one
  packaged file that changed; the freeze golden is regenerated (ADR-0200 amendment of
  2026-10-18). A session still live at a browser exit leaves one saved group, which no extension
  API can reach. An unpacked extension runs the new code only after a reload of Ghostlight in
  `chrome://extensions`; the owner reloaded it on test-01.
- **rc-1** ([LEDGER](tasks/rc-1/LEDGER.md)): opened 2026-10-18. A product-owner pass and a visual
  coherence pass ran before the release candidate, a skeptic checked every finding, and the
  confirmed ones landed in five lanes: the install scripts, uninstall and doctor; what agents see
  (a call with no tab among several is refused, page refusals say what to do, each connection
  names the app that started it); the window's words, type and widths; the frame and the tray;
  and the extension's popup. The docs lane made the CHANGELOG, the notes, the trust pages and the
  release steps match. Integrated, deployed in place on the owner's machine and checked live on
  2026-10-18. The release-candidate rehearsal then passed on 26e48ad0 (run 39173522935, no tag,
  sign or publish). Its bundle verifies with `scripts/check-release-candidate.ps1` (version
  2.0.0, 16 artifacts), and `winget validate` accepts its manifest.
- **request-1** ([LEDGER](tasks/request-1/LEDGER.md)): opened and closed 2026-10-18.
  [ADR-0202](adr/0202-what-a-request-carries.md), what a request carries. Every request now
  carries one id from intake to audit, its observed source and declared channel, and its own
  findings. The channel switches are back in policy. Deployed and checked live.
- **ux-2** ([LEDGER](tasks/ux-2/LEDGER.md)): opened 2026-10-18, closed 2026-10-19. The designer
  and the specialist walked the deployed build again; this batch carried what was still open and
  what the walk found, under the owner's rule that teal means live. Deployed and checked live; the
  owner validates it.
- **arch-1** ([BOOTSTRAP](tasks/arch-1/BOOTSTRAP.md)): opened 2026-10-19.
  [ADR-0203](adr/0203-one-owner-for-each-fact.md), one owner for each fact. An architecture sweep
  of the 2.0 tree confirmed 102 findings of re-derived, duplicated and singleton facts; the ADR
  turns them into thirteen decisions and seven lanes. The most serious: after a browser restart a
  remembered tab id could name one of the person's own tabs. Every slice gated candidate has
  landed, integrated with the full gate green, and so has a follow-up of eight (one Ghostlight
  toolset per agent process, so the Claude Code plugin works with or without the installer's
  entry). Deployed and checked live on 2026-10-19 (62bfa224). The 16 slices gated none are
  deferred until after 2.0, at the owner's request to spend only on real value.

The earlier lane ledgers in that folder (`service.md`, `extension.md`, `connectors.md`,
`switch.md`) keep their lanes' record; the switch's remaining slices continue in the seams ledger.
2.0.0 was released from this work on 2026-10-19 (Release checklist below).

## Release bar

2.0.0 shipped on 2026-10-19 by the owner's go. The owner skipped the 1.x upgrade check, and the
Windows restart and sign-out check is still open (Release checklist). The bar as it stood:

2.0 ships when it passes checks that prove its promises, never on a date or after a waiting
period: clean installs on Windows and the Linux test machines, a 1.x upgrade on another machine,
and, on Windows, a restart and a sign-out with an agent session, the browser and the window open,
which must show no shutdown block and leave exactly one service. The 1.x user base is three
reachable people and they are told directly. 2.0 settles the seams (executables, relay and adapter
protocols, policy schema, agent entries, install home) and 2.x rebuilds internals lean behind them.
The seams stay stable by default; one changes only when a separation-of-concerns analysis shows a
meaningful architecture change needs it, never because of a freeze (ADR-0197 amendment of
2026-10-18).

The release path is simple (the owner, 2026-10-19): the scope is closed, so only fixes for something
a person would hit go in, each one commit with its test. One bundle, built by the release workflow
from `dev`, is what every machine tests, and `v2.0.0` is tagged on that exact commit. A fix after
that rebuilds the bundle and re-runs only the check it touches. The steps of the cutover are in
`docs/RELEASE.md`.

## CI

The last green run is 39360079270 on `dev` e789913b (2026-10-19), the public state of 2.0.0:
every job, including the Rust, extension and page-asset gates, and every process and
real-browser journey on Linux and Windows. The run on `main` for the same commit (39360083656)
failed once in the Windows process journey: a cut-off read's 8 s default timeout could expire
before the test killed the service on a slow runner. f5d8507a gives that read the longest
timeout. Since the CI review of 2026-10-19 a newer push no longer cancels a run in progress, and
cargo audit runs daily (audit.yml) and in the release workflow.

## Release checklist

Done on 2026-10-19 (custody record:
[testing/release-2.0.0-2026-10-19.md](testing/release-2.0.0-2026-10-19.md)):

- The 2.0 bundle: release run 39347181245 on 6a41327f, verified with
  `scripts/check-release-candidate.ps1` (2.0.0, 16 artifacts). It differs from the bundle every
  machine tested (39323041650 on eba78013) only in docs, and its extension ZIP is the same
  (`ee224962...`). `v2.0.0` is tagged on 6a41327f.
- Tested on that code: the owner's Windows machine, CachyOS (linux-2) and Bluefin (test-02,
  `docs/testing/bluefin-2.0-bundle-2026-10-19.md`). Bluefin's finding, that status does not say
  what accept answers a prompt with, is kept for 2.0.1 (the owner and the coordinator).
- sylin.org's privacy page carries the 2.0 policy (website 9117a8e).
- The cutover (`docs/RELEASE.md`), steps 1 to 5 and 7 to 9, run by the coordinator at the owner's
  go. GitHub, npm, the Chrome Web Store (2.0.0 at 100%, from the approved staged submission) and
  the MCP Registry serve 2.0.0. Step 4 proved the public install on the owner's Windows machine:
  both one-liners, the restore route, the launcher, and a real task. The public state (e789913b)
  is on `dev` and `main`, the website shows 2.0.0 (website 969866d), and
  `scripts/check-public-surfaces.ps1 -Online` passes. The registry was published before `main`
  moved, because the online check needs it; the website rebuild came last.
- WinGet: the validated 2.0.0 manifests are in microsoft/winget-pkgs#456409, merged on 2026-10-20. The store listing keeps its 1.x images until the owner replaces them.

Now:

- On Windows, `ghostlight-desktop.exe` vetoed every shutdown, restart and sign-out in 2.0.0: Windows
  logged it on the owner's machine on 2026-10-20 (Winsrv event 10001). Tao's hidden thread event
  target window answered `WM_QUERYENDSESSION` with FALSE. ac0026df takes Tao 0.36.0, which fixes it
  (ADR-0120 amendment), and the Windows desktop journey now asks every window of each process, as
  Windows does. Deployed on the owner's machine, which runs a development build again, and checked:
  every window of the live shell allows the end. The owner's restart at 14:16 on 2026-10-20 went
  through: Windows recorded no veto from Ghostlight, and afterwards one service ran, started by an
  agent, with the shell in the tray. The owner holds it for 2.0.1 with more (below).
- 2.0.1, the owner's scope (2026-10-20): the shutdown fix (ac0026df); Alpine and other musl systems
  from the release (linux-claude, linux-2 L2-01 and L2-14); `browser_dialog` status naming a
  prompt's default, which the extension carries through, with the wording alone for an extension
  older than 2.0.1 (an extension update and a store review); and the `browser_tabs` action
  wording. Added by the owner later that day: `browser_navigate` accepting the advertised default
  `reuse` beside `new_tab`, which a client that fills schema defaults sends (Claude Code began to),
  and ureq 3 for the managed-policy fetch, which removes the `webpki-roots` audit exception. It ships
  when these land, at the owner's go.
- 2.0.1, landed on `dev` on 2026-10-20 beside that scope, each from a live finding:
  - The one-line installers never run the GitHub CLI (53ce4654). In Windows PowerShell 5.1 a `gh`
    that was installed but not signed in stopped `get.ps1` before it installed anything, and the
    check could never stop a bad install. The owner wants the installers to need nothing from
    GitHub but the files (MEMORY.md). At the owner's go, `main`, which the short links serve, took
    those two scripts at once (2303c404, merged back into `dev` as a3686360). Test-04 then ran
    the public one-liner in Windows PowerShell 5.1 with a signed-out `gh`: it installed, exit 0.
  - The install never chooses another app's container for an agent (4fac15a7, ADR-0197 amendment
    of 2026-10-20): VS Code, never started on leo-desktop-02, was registered inside the Codex app's
    container and called connected.
  - The extension never waits on a page that shows a dialog (2f9c11f9, ADR-0168 amendment of
    2026-10-20). A prompt left open in a controlled tab stalled every command after a reconnect, a
    pause or a release until someone closed it (test-03, store extension 2.0.0). Deployed on the
    owner's machine and checked live with the owner's reload: a prompt stayed open across a browser
    relay reconnect, and status, a new tab and the answer all worked at once. With the store's 2.0.0
    extension, answering or cancelling the dialog ends the stall; the release notes say so.
  - Alpine is proven: release run 39508116197 on 627940a8 passed all ten jobs, and test-03 installed
    that bundle with `get.sh` and rang the desk bell in its Chromium (linux-2 L2-14).
  - The Claude Desktop bundle ships with 2.0.1 instead of ending at 2.0.0 (ADR-0197 amendment of
    2026-10-20), under the owner's rule that rules serve the end user (MEMORY.md).
- The 2.0.1 candidate, at the owner's go: version 2.0.1 stamped (fb72dea4), notes in
  `release/notes-v2.0.1.md`. Release run 39517667200 on dc89cd60 passed all ten jobs, the installs
  from the bundle on Alpine and Ubuntu 22.04 and 24.04 included. `scripts/check-release-candidate.ps1`
  verifies the bundle (2.0.1, 19 artifacts, the MCPB among them), and its extension ZIP
  (`bddfb37a...`) is the one the freeze golden records. Deployed on the owner's machine (a real
  task passed on 2.0.1). The four test machines upgraded their 2.0.0 installs to it, and every
  check passed: CachyOS (`testing/cachyos-2.0.1-bundle-2026-10-20.md`), Bluefin
  (`testing/bluefin-2.0.1-bundle-2026-10-20.md`), Alpine (`testing/alpine-2.0.1-bundle-2026-10-20.md`)
  and leo-desktop-02 (`testing/windows-2.0.1-bundle-2026-10-20.md`, where VS Code is no longer
  registered in Codex's container and every window of the shell now allows a restart). An agent
  rang the desk bell on each Linux machine with a 2.0.0 extension. Until a browser runs the 2.0.1
  extension, status cannot say what a prompt's field holds, so an agent not told to accept may
  answer with its own text (Bluefin).
- The 2.0.1 extension (`bddfb37a...`) was submitted to the Chrome Web Store for review as a staged
  publish on 2026-10-20, and the owner cancelled that review on 2026-10-21 ("let's get everything
  fixed first"). The store still serves 2.0.0 at 100%. The next submission is the refactored
  extension once the pass is done (the package lane's B-final), at the owner's go (ADR-0200
  Decision 16).
- 2.0.1 is held for a maintainability pass (owner, 2026-10-20: "Refactor first, ship later"), so
  the candidate above is not cut over. arch-1 reopens with a second sweep: 100 findings over every
  area, each checked by a skeptic (57 confirmed, 43 partial, none refuted), and the 15 defects it
  found go first ([the sweep](tasks/arch-1/sweep-2026-10-20/README.md), the ADR-0203 amendment
  of 2026-10-20). Its lanes run on five machines, each lane in its own files
  ([BOOTSTRAP](tasks/arch-1/BOOTSTRAP.md#the-second-sweep-2026-10-20)). After the pass, a new 2.0.1
  candidate is built and re-verified on every machine. Then, at the owner's go, the staged store
  submission is cancelled for the refactored extension, and the cutover of `docs/RELEASE.md`
  follows.
- The second sweep's defect slices fixed the after-Quit reconnect too (S-D3, the owner's design of
  2026-10-20, the ADR-0199 amendment of that day). A waiting edge registers a wake endpoint, and a
  ready service wakes every one of them, with the backoff kept as the fallback. The edge and the
  relay are now relay revision 5, and the service admits 4 and 5. The extension is unchanged.
  Proven live on 2026-10-21. On leo-desktop-02, 54 s after a Quit, an agent call attached the open
  Chrome 57 ms after the service was ready, with no second window. On Bluefin, 48 s after a Quit,
  the browser edge attached 27 ms after the start, with no `browser_startup_manual`.
- The night of 2026-10-21 fixed two more defects found while proving S-D3. S-D4: a start
  relaunches itself once, so an agent host that kills its MCP server's process tree on exit (the
  Copilot CLI on Windows) no longer takes the service and its window with it (ADR-0199 amendment
  of 2026-10-21; `taskkill /T` on leo-desktop-02 now leaves both running). P-D2: an extension
  command the service cancelled while it waited no longer runs late, which had opened tabs and a
  window after a held dialog cleared (f5c01ca6).
- The owner's live test of 2026-10-21 added two changes. L-D6: a script that threw tells the agent
  what it threw, bounded, and the audit keeps only that it threw (28ea74b3). P-D3: a tab group's
  title is the ghost emoji and the agent's name instead of `Ghostlight - `, and the extension
  retitles a group 2.0.0 named rather than opening a second one (ADR-0098 amendment of
  2026-10-21). P-D3 changes three packaged extension files, so the next candidate carries it.
  The owner checked the tab strip on Windows on 2026-10-21 ("it's working beautifully"), and the
  color ghost renders on CachyOS (KDE Wayland), on Alpine (both with Noto Color Emoji) and on
  Bluefin (GNOME Wayland, its default fonts); leo-desktop-02 is owed. The Linux browsers run 2.0.0
  extensions, so the old group's rename was seen on none of them. dev
  73d5cc46 is deployed on the owner's machine and was checked live on the local 2.0.1 extension:
  the desk with its prompt and a held dialog, the three forms, diagnose and wait, history, a
  script, a screenshot and a kept close, plus P-D4 and P-D5. The morning's extension check had run
  on a store 2.0.0 extension left installed beside the local build; the owner removed it. Found
  while doing P-D3, P-D4: the extension counts a text limit in characters, as the service does, so
  text with emoji within a tool's limit is no longer refused. P-D5, found checking P-D4 live: text
  cut inside an emoji (a long tab title, a script's result) dropped the browser connection for
  every session; the extension now sends text well formed, and the service skips a frame it cannot
  decode.
- 2.0.1 is published (2026-10-22): the GitHub release `v2.0.1` (Latest), npm `ghostlight@2.0.1`,
  the Chrome Web Store adapter 2.0.1 at 100%, the MCP Registry record and `main` at the release.
  WinGet's manifest is microsoft/winget-pkgs#458539, held by a Defender machine-learning false
  positive (Wacatac.C!ml) that Microsoft is reviewing (WDSI d5b9b3cd). The custody record is
  [testing/release-2.0.1-2026-10-22.md](testing/release-2.0.1-2026-10-22.md); the public install
  is proven on Alpine and Windows, with Bluefin to follow when its session is back.
- The 2.0.1 extension (`aaa234b1...`, from candidate run 39669867246) was submitted to the Chrome Web
  Store on 2026-10-22 through `scripts/publish-extension.ps1`, at the owner's go: uploaded, then
  submitted for review as DEFAULT_PUBLISH, so it goes public when Google approves it. State:
  PENDING_REVIEW; the store still serves 2.0.0. The release and `main` follow it.
- Friendly usage, strict authority ([ADR-0205](adr/0205-friendly-usage-strict-authority.md),
  2026-10-29, owner direction). A Sonnet 5.5 session failed 6 of 18 calls on SharePoint and
  ServiceNow: three refusals of clear calls and three false answers. On `dev`: `language/values.rs`
  reads values the way CSS does (numbers, units, keywords, omitted values from the call or the
  page), ignores what does not apply and reports `ignored` and `adjusted` facts; the schema admits
  the forms a validating client would otherwise block; a page that changes during a read is
  `page_changed` and the read runs once more; a navigation with a reported commit lands as
  `loading`; a page scroll moves what actually scrolls and reports `moved` and `at_end`. Deployed
  on the owner's machine and checked live through the installed edge (the desk and reading demo
  stages, a page whose document does not scroll). The extension is unchanged. Not pushed.
- The 2.0.1 candidate is release run 39715160480 on 8e185d67 (2026-10-22), every job green. Test
  machines upgraded and passed: Bluefin (588ddd95), Alpine (48ef55fd, 657495a3; the 2.0.0 extension's
  dialog stall needs a browser restart, now in the notes) and leo-desktop-02 (3ff45599, 55cb0756, with
  the Store-build Claude Code name fixed in 8e185d67). CachyOS was offline after a power loss. The
  extension zip to submit is the one the freeze record names (run 39669867246); no packaged file has
  changed since. Next, the owner's: the store resubmission, the release, then `main`.
- The second sweep closed on 2026-10-21, with every arch-1 row landed but three the owner deferred
  past 2.0.1 to save tokens: S-D9 (a client line over 8 MiB is answered), A-D2 and A-D3 (page-asset
  role and settle fixes). All three landed later the same day after all. The lanes split the
  remainder five ways after tests-1 closed. Next: a new
  2.0.1 candidate, re-verified on every test machine, then the owner's store resubmission and
  cutover.
- tests-1 opened on 2026-10-21 (ADR-0204, the owner's mandate of meaningful tests): the 2,851
  Rust tests are cut to at most 500 before the 2.0.1 candidate, in five lanes on the test
  machines and the coordinator's ([BOOTSTRAP](tasks/tests-1/BOOTSTRAP.md)). arch-1's refactor
  slices wait for it; core-work's W-D4 and W-D5 and install's I-D4 go first. It closed the same day
  at 462 Rust tests, with `tests/guards/test-budget.test.mjs` holding the ceiling in CI. The cut
  found and fixed S-D7, S-D8, P-D6, A-D1, A-D4, G-D1 (a trailing dot stepped around deny lists),
  G-D2 to G-D4, L-D7, U-D5 and U-D6. The owner, on token spend, deferred S-D9, A-D2, A-D3 and the
  JavaScript ceiling past 2.0.1; the page assets' plan (69 of 275 under a proposed 75) is in the
  adapter ledger.
- Release order for 2.0.1, from test-01's reading of the 2.0.0 package (2026-10-21): publish the
  2.0.1 extension before the service, or with it. A 2.0.0 extension beside a 2.0.1 service matches
  groups by exact title, so it opens a ghost-titled group beside the agent's old "Ghostlight - ..."
  group, and its popup names the agent with the ghost in front, until the store update arrives and
  the next open merges them (ADR-0098 amendment of 2026-10-21). The 2.0.1 extension beside a 2.0.0
  service keeps the old title and names the agent right, so the extension first costs nothing.
  Publishing is the owner's go.
- Owed before the new 2.0.1 candidate: the changelog and `release/notes-v2.0.1.md` take the
  second sweep's user-visible fixes. About twenty landed after the changelog's last entry
  (7cecdda7), and of those only P-D3, P-D4 and P-D5 wrote theirs. The notes also tell people that an
  old "Ghostlight - <agent>" entry in the saved tab groups bar can be removed by hand: Chromium
  saves a group still open when the browser exits, no extension API can rename or delete a saved
  group, and test-02 found one beside the ghost group after a power-off.
- Second sweep state after that night. core-language and adapter are done, and so is the policy
  lane. package: EXT-6, EXT-12 and B-final are next. core-work: W-D4 is next: after a Quit, closing
  a handle the Quit released says "That tab was already closed." while the tab stays open.
  install: INST-5 is next. core-service: C19c-s is next, then the cli journey's no-browser
  language checks, which lost their call when a session with no recording began answering at
  once. dev 0b04e5b5 passed the full gate and is deployed on the owner's machine. One look is
  owed: a window seen once on leo-desktop-02 stayed at "Starting" after an update. It did not
  happen again there or on the owner's machine.
- The public one-liner on the real installations, which the cutover went on without: done on
  CachyOS (test-01, `docs/testing/cachyos-public-2.0.0-2026-10-20.md`; a same-version candidate
  service is kept until `--restore`, so a second candidate of one version is installed after an
  uninstall or with `--restore`), on Bluefin (test-02,
  `docs/testing/bluefin-public-2.0.0-2026-10-20.md`; its images already equalled the release) and
  on a second Windows machine (leo-desktop-02, `docs/testing/windows-public-2.0.0-2026-10-20.md`;
  over 1.3.5 leftovers and again after an uninstall, both Ready, the first agent task skipped by the
  owner).
- Owner: step 6, the clean-profile run of the public path on Windows and Linux.
- Once WinGet's index lists 2.0.0: step 10, the README and installation guide lines that say WinGet
  carries 2.0, and a `main` fast-forward.
- `scripts/publish-website.ps1` clones the website into the system temp folder, outside the folder
  whose Git configuration names the owner's GitHub account, so Git Credential Manager waited for an
  account choice. The step's commit (969866d) was pushed from the owner's website checkout
  instead. A clone under the repository would inherit that configuration.

Still open from before the tag:

- On Windows: a sign-out with an agent session, the browser and the window open: no shutdown block,
  and one service afterwards. The restart passed on 2026-10-20 with ac0026df, not with 2.0.0. One `.mcpb` install in Claude Desktop from the
  release, and the extension in Chrome and Edge.
- The owner's look at the window on a light-mode Windows account at 100% and 150%.
- The three 1.x users told directly, and the owner's look at the website's 2.0 pages.

After 2.0.1: ADR-0203's sixteen deferred slices and the CI review's deferred items.
