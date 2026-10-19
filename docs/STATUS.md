# STATUS -- Ghostlight 1.3.14 published; 2.0 release candidate in preparation

This page is the current state and nothing else. Every earlier dated section, from the 1.0 rebuild
through 2026-10-16, moved verbatim to [history/status-through-2026-10-16.md](history/status-through-2026-10-16.md).
Decisions live in [adr/](adr/README.md), standing preferences in [MEMORY.md](MEMORY.md).

## Published

- Release 1.3.14 is the latest public release, with the unchanged Chrome adapter 1.3.12 in the
  Chrome Web Store (`docs/public-status.json`, `server.json`, `compatibility.json`).
- 1.3.15 is not released. It was submitted for staged Chrome review (PENDING_REVIEW at the last
  recorded read) with a non-public GitHub draft, and the release posture below says 1.x gets no
  patch releases. The owner withdrew it on 2026-10-16: the draft is deleted and the `v1.3.15` tag
  stays as history. The owner cancelled and pulled the Chrome Web Store item on 2026-10-17, so the
  listing is clear for the 2.0 staged submission.
- Nothing from the 2.0 break set is published. No `v2*` tag exists.

## Branches

Only `main` and `dev`, locally and on the remote (AGENTS.md, Branches). `main` holds the published
1.3.14 product. `dev` holds all 2.0 work. `main` advances to the 2.0 tree in the cutover sitting
(ADR-0200 Decision 16 and its amendments), because the short install commands fetch their scripts
from `main`. Older branch heads are kept as `archive/` tags.

## 2.0 in flight

`dev` carries the 2.0 break set of
[ADR-0197](adr/0197-contained-seams-and-the-2-0-break.md), the one-service-and-thin-shells design of
[ADR-0199](adr/0199-one-service-thin-shells.md) and the agent experience of
[ADR-0198](adr/0198-agent-experience.md). The product version is 2.0.0 everywhere it is stamped.
The three ledgers below are the record of what is done; read the RESUME HERE block in each.

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
The release candidate is next: the checklist below, then the owner's release order.

## Release bar

2.0 ships when it passes checks that prove its promises, never on a date or after a waiting
period: clean installs on Windows and the Linux test machines, a 1.x upgrade on another machine,
and, on Windows, a restart and a sign-out with an agent session, the browser and the window open,
which must show no shutdown block and leave exactly one service. The 1.x user base is three
reachable people and they are told directly. 2.0 settles the seams (executables, relay and adapter
protocols, policy schema, agent entries, install home) and 2.x rebuilds internals lean behind them.
The seams stay stable by default; one changes only when a separation-of-concerns analysis shows a
meaningful architecture change needs it, never because of a freeze (ADR-0197 amendment of
2026-10-18).

Release order (`docs/RELEASE.md`, The 2.0 cutover; `docs/release/store-disclosure-2.0.md`;
ADR-0200). There is no prerelease tag (the owner, 2026-10-19): the release candidate is the bundle
the release workflow builds from the candidate commit. First the install proof on that bundle on
Windows PowerShell 5.1, PowerShell 7 and a Linux shell, a 1.x upgrade on another machine while the
store still serves 1.x, the 2.0 privacy text on the public page, the staged store submission and
its approval. Then in one sitting: `v2.0.0` as latest, `main` fast-forwarded, the npm launcher
published, the frozen install strings proved, the staged extension published, a clean-profile run
of the public path on Windows and Linux, the public-state commit with a second fast-forward of
`main` and a website rebuild, the MCP Registry record and the WinGet manifests, and once
winget-pkgs merges them, the README and installation guide lines that say WinGet carries 2.0. Not
before approval.

## CI

The last green run is 39278943156 on `dev` eb038961 (2026-10-19): every job, including the Rust,
extension and page-asset gates, and every process and real-browser journey on Linux and Windows.
It carries arch-1 (ADR-0203) integrated. Since the CI review of 2026-10-19 a newer push no longer
cancels a run in progress, and cargo audit runs daily (audit.yml) and in the release workflow.

## Release checklist

Each line names who does it: the coordinator (the agent session that integrates and deploys) or
the owner. The ledgers keep the detail; this is what still gates the release.

Before the release candidate's bundle:

- linux-claude: linux-2 Part 2 on CachyOS, from the release workflow's bundle (the go was sent on
  62bfa224).
- Coordinator: one MCPB install in Claude Desktop from the rehearsed unit, on a clean Windows test
  machine. The rehearsal, the candidate check and `winget validate` passed on 26e48ad0.
- Coordinator: a Windows Chrome and Edge check of the extension: a session end with preserve-tabs
  on, a reuse, and the popup states.
- Owner: JQ3, keep or drop the extension's host permissions, recorded as a decision.
- Owner: the installer's wait times (90 s and 120 s), one timed cold run or "keep" recorded.
- Coordinator: the release workflow's bundle from the candidate commit, verified with
  `scripts/check-release-candidate.ps1`. Nothing is tagged or published.

Between the bundle and the cutover:

- Coordinator: the install proof on the bundle (`GHOSTLIGHT_FROM`) on Windows PowerShell 5.1,
  PowerShell 7 and a Linux shell, and clean installs from it on a Windows test machine and the
  Linux test machine.
- Coordinator: a 1.x upgrade on another machine while the store still serves 1.x
  (`docs/RELEASE.md`, The 2.0 cutover, step 3).
- Owner: on Windows, a restart and a sign-out with an agent session, the browser and the window
  open: no shutdown block, and one service afterwards.
- Owner: the daily machine on the candidate build, one check on a light-mode Windows account at
  150% (frame, height, tray), and the desktop shell's checkpoint 2.
- Owner: tell the three 1.x users directly: remove the 1.x Claude Desktop extension or install the
  2.0 `.mcpb`, and do not run `npx ghostlight` until the cutover.
- Owner: the sylin.org redirects `/ghostlight/get.ps1` and `/ghostlight/get.sh` to the scripts
  on `main`, and the website's 2.0 pages (separate repository).
- Owner: what a store reviewer is offered while the item is in review. With no prerelease, the
  public command installs 1.x until the cutover (`docs/release/store-disclosure-2.0.md`, Notes for
  certification).
- Owner: the 2.0 privacy page published, then the staged store submission with the remote-code
  answer (No) and the recaptured assets, and its approval.

The cutover, in one sitting (`docs/RELEASE.md`, The 2.0 cutover):

- Owner: `v2.0.0` as latest, `main` fast-forwarded, the npm launcher, the frozen strings and the
  restore commands, and the store publish.
- Coordinator: the clean-profile run of the public path on Windows and Linux.
- Coordinator drafts, owner pushes: the public-state commit, the second fast-forward of `main`
  and the website rebuild.
- Owner: the MCP Registry record and the WinGet manifests.
- Coordinator drafts, owner pushes: once winget-pkgs merges the 2.0.0 manifest, the README and
  installation guide lines that say WinGet carries 2.0, and a fast-forward of `main`.

Tags, publication and store actions are the owner's call.
