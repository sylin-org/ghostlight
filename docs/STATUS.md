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
  for one. Owed live checks (fresh homes on Windows and Linux, the pinned release-candidate
  install) and the store submission are listed in the ledger.
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
  ledger, the record). Owed: the release candidate's pinned install, and the items the ledger
  marks "seen and left" for the owner.

The earlier lane ledgers in that folder (`service.md`, `extension.md`, `connectors.md`,
`switch.md`) keep their lanes' record; the switch's remaining slices continue in the seams ledger.
The release candidate is next: the CHANGELOG [2.0.0] heading, a local test, then the owner's
release order below.

## Release bar

2.0 ships when it passes the bar the owner set on 2026-10-14, not on a date: clean installs on
Windows and the Linux test machines, a 1.x upgrade on another machine, and a week of daily use
with no shutdown blocks and no second service. The 1.x user base is three reachable people and
they are told directly. 2.0 freezes the seams (executables, relay and adapter protocols, policy
schema, agent entries, install home) and rebuilds internals lean in 2.x behind them.

The week of daily use started on 2026-10-16, once the shell was complete. Only a change to the
service or shell lifecycle (start, stop, presence, session end, update) restarts it; install-time
work does not (the owner delegated this call to the coordinator, 2026-10-16).

Release order (`docs/RELEASE.md`, The 2.0 cutover; `docs/release/store-disclosure-2.0.md`;
ADR-0200): the `v2.0.0-rc.1` GitHub prerelease, the pinned install proof on Windows PowerShell 5.1,
PowerShell 7 and a Linux shell, the staged store submission, approval and the live privacy text,
then in one sitting `v2.0.0` as latest, `main` fast-forwarded, the npm launcher published, the
frozen install strings proved and the staged extension published. Not before approval.

## CI

The last recorded green run is 38936325057 on `dev` ec0f61f4 (2026-10-16): all 11 jobs, including
the Rust, extension and page-asset gates, supply chain, and every process and real-browser journey
on Linux and Windows. Commits after it are checked locally by the lane gate; CI runs when the owner
pushes.

## Owed by the owner

- Publish the sylin.org redirects `/ghostlight/get.ps1` and `/ghostlight/get.sh` to the scripts on
  `main` (separate repository).
- The dashboard submission for 2.0 (staged, never published ahead of the release order), the
  remote-code answer (No), the recaptured store assets, and the public privacy page carrying the
  setup profile text before publication.
- The owner's checkpoint 2 on the desktop shell, and the live checks the ledgers list under
  "Owed live checks".
- The release order above, in the owner's hands: tags, publication and store actions are the
  owner's call.
