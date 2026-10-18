# rc-1: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `landed` (with commits), `blocked`
(with the reason).

## RESUME HERE

Every slice has landed and the integration review passed (2026-10-18): the full gate is green, and
its one finding (the notes over-claimed the tab rule) is fixed in
`fix(language): shared-session agents open their own tab`. The owner's four held items are
decided below. Next: the in-place deploy and live check, the push, the release-candidate rehearsal
on a green head, and the items under "Before rc.1" and "Before v2.0.0". The request pipeline
rebuild (ADR-0202) follows in [request-1](../request-1/LEDGER.md) before rc.1.

## Slices

| Slice | Lane | Status |
| --- | --- | --- |
| a1-get-ps1-reads-real-releases | frontdoor | landed (`fix(install): the Windows one-line installer reads a real release's checksums`) |
| a2-uninstall-says-what-it-leaves | frontdoor | landed (`feat(uninstall): uninstall says what it removed and what it keeps`, `fix(uninstall): a partial uninstall and its dry run say what it really removes`) |
| a3-private-views-are-named-not-blamed | frontdoor | landed (`fix(install): an app container is found by its effect, and its private view is named, not blamed`) |
| a4-adaptive-small-promises | frontdoor | landed (`fix(browser): a service inside an app container leaves browser startup to the person`, `fix(install): the install reads browser profiles only while setup is incomplete`, `fix(install): a Linux install names the full path when ghostlight is not on PATH`, `fix(doctor): doctor lists the window program`, `fix(install): a config write leaves no temporary file behind`) |
| b1-no-tab-never-guesses | agent | landed: with several tabs a call that names none is refused and lists them; no per-caller marker exists (ADR-0198, amendment of 2026-10-18) |
| b2-page-refusals-say-what-to-do | agent | landed: a hidden, disabled, read-only or non-text control is refused in plain words, says nothing was clicked or typed, and names one step that works (ADR-0198, amendment of 2026-10-18) |
| c1-screen-readers-and-failures | window | landed (`fix(workbench): the window is calm for a screen reader, and a failure looks like one`): one status line says each settled action, at most one a second; failed and kept rows and the card say so in words; a destination that cannot be read says so in place, with Try again; both dialogs hold focus and give it back |
| c2-one-legible-type-and-colour-system | window | landed (`fix(workbench): one legible type and colour system on every tab`): every word is at least --ink-4 (5.6:1) and about 11px; one eyebrow, one data tag and one page title; Policy's restrictions no longer collide with the kit's Settings grid; summaries, the agent pane's mascot and the About orbs fixed |
| c3-one-vocabulary-and-plain-status | window | landed (`fix(workbench): the window, the tray and status say the same thing in plain words`): one count, the agents At a glance groups, on the band, the pill, Status, About, the tray and `status`; Pause agents and Resume agents everywhere; Status says Ghostlight, Browser, Rules and History in plain words; a pause outranks no browser, and a browser that starts on request reads Ready; Policy says the open case once and a setting alone applies; the Windows first-close notice names the hidden icons and the Start menu; Closing tabs names Preserve Ghostlight tabs (ADR-0199 and ADR-0187 amendments of 2026-10-18) |
| c4-rows-and-widths | window | landed (`fix(workbench): the activity list keeps its meaning at every width, and the window fits a laptop half`): a row names one capability, the most consequential, and a flow takes its steps' (no more LOCAL); the band's facts give way below 1000px and the tabs take their own line below 900px; every destination holds at 720 and 600 CSS px; Settings > This screen offers Text size Standard / Large. The window minimum is 720 (lane native, `crates/desktop/src/window.rs` `WINDOW_MIN_SIZE`, already 720 x 620) |
| d1-native-frame-fits-the-person | native | landed (`feat(tray): the tooltip names the state and says when no browser is connected`, `feat(desktop): the frame and the tray fit a light-mode laptop`): the title bar is dark in light mode; a new window opens no taller than the primary monitor less 96 px (624 high on 1280x720), centered above the taskbar; the minimum width is 720; the Windows tray draws the 16 px ghost at 16 px and the 32 px one at 24 and 32, and Linux always draws the 32 px one for its host to size, inks it on a light taskbar (re-read every 2 s) and rings a 3 px dot in the taskbar's ground; the executable carries 16/32/48/128; the tooltip leads with the band's word and says "No browser connected" (ADR-0199 amendment of 2026-10-18). Owed: the light-mode 150% check under Before v2.0.0 |
| f1-extension-popup-and-kit | extension | landed (`fix(extension): the popup tells Paused apart and reaches the window`) |
| b3-every-connection-knows-its-source | provenance | landed (`feat(provenance): every connection knows the app that started it`): the service reads the app that started each MCP connection from the peer process's ancestors, recognizes an agent's app by where it is installed, and sets the client's claim beside it in the window, history and audit, never as authority (ADR-0161, amendment of 2026-10-18) |
| e1-the-rc-commit-tells-the-truth | docs | landed (`fix(cli): the manual page and completions describe 2.0`, `docs(release): the rc commit tells the truth`, `docs(install): WinGet says it serves 1.x until its manifest merges`): CHANGELOG [2.0.0], the 2.0.0 notes and new rc.1 notes, README, SECURITY, the trust pages, PRIVACY, the npm README, ROADMAP, RELEASE's cutover, the store steps, STATUS's release checklist and INTENT match the tree; README and the installation guide carry the WinGet caveat, and the cutover's step 10 replaces it once winget-pkgs merges |

## Decided by the owner (2026-10-18, "Proceed.")

- `main`'s 1.x `scripts/get.ps1` is patched (`fix(install): the Windows one-line installer reads a
  real release's checksums` on `main`, pushed and live through sylin.org's redirect), checked
  against the real v1.3.14 release in Windows PowerShell 5.1 and PowerShell 7.
- The popup fixes are folded into the 2.0 extension: slice f1, the ended-state alignment
  (`fix(extension): an ended session holds the popup's lantern, as the tray and the window show
  it`) and the ADR-0200 amendment of that day.
- The accent rule is "teal means live": teal for work happening and the one primary action per
  view; everything at rest is ink.
- The "Build release candidate" rehearsal is approved and passed. The first run (39162168362)
  failed on Windows: a step ran the fixed-installation journey with release binaries on GitHub's
  elevated runner, where they rightly refuse to start; that check moved to CI's every-push journeys
  (`ci(release): the Windows fixed-installation check runs in CI on every push, not in the release
  build`). The second run (39173522935, at `26e48ad0`) passed every job. On the owner's word the
  coordinator downloaded its bundle: `scripts/check-release-candidate.ps1` verified version 2.0.0,
  the source commit and 16 artifacts, and `winget validate` accepted the WinGet manifest. The
  `.mcpb` install is left for the clean Windows test machine.

## Before rc.1, not code

- JQ3: keep or drop the extension's host permissions, recorded as a decision.
- The installer's wait times (90 s and 120 s): one timed cold run, or "keep" recorded.
- A Windows Chrome and Edge check of the unfrozen extension: a session end with preserve-tabs on,
  a reuse, the popup states.

## Before v2.0.0, not code

- Clean installs on a Windows test machine and the Linux test machine from the rc build.
- A 1.x upgrade on another machine, while the store still serves 1.x.
- On Windows, a restart and a sign-out with an agent session, the browser and the window open: no
  shutdown block, one service afterwards.
- The owner's daily machine on the rc build.
- The 2.0 privacy page published before the staged submission; the website's 2.0 pages.
- One check on a light-mode Windows account at 150% (frame, height, tray).

## Notes

- a1 (install-1, install-9): `scripts/get.ps1` read SHA256SUMS through
  `(Invoke-WebRequest ...).Content -split`. GitHub serves that file as
  `application/octet-stream`, so `Content` is a byte array in Windows PowerShell 5.1 and in
  PowerShell 7, the split gave one-byte lines, and every real install stopped with "SHA256SUMS does
  not bind exactly one ghostlight-x86_64-pc-windows-msvc.exe." before downloading anything. The
  test mock returned a string, so CI never saw it. The script now saves the file and reads its
  lines, and its body runs in a script block of its own so `irm | iex` leaves the person's
  session as it was. `tests/installer-checksums.ps1` parses a byte payload offline in both shells
  and, in CI on Windows, the real v1.3.14 SHA256SUMS. `get.sh` already saves with `curl -o` and
  reads the file; its test now serves a release-shaped file. `main`'s 1.x `scripts/get.ps1` has
  the same line (held for the owner above).
- a2 (install-7, docs-9): a real `ghostlight uninstall` now prints one summary from
  `language/setup.rs` (`uninstall_lines`, beside `install_lines`): the agents and browsers it
  removed Ghostlight from, whether the running Ghostlight stopped, the home it kept and that
  deleting it removes the programs, audit history and settings, and that Ghostlight in Browser
  stays until removed from the browser. The per-registration report lines print only with
  `--dry-run`, which now also names each browser connection and the PATH entry (or
  `~/.local/bin/ghostlight`) it would remove, read from the same check the uninstall acts on
  (`CommandPath::removable`). What uninstall removes did not change. The installation guide's
  uninstall section says the same and how to delete the home.
- a3 (trust-9, window-12, install-11, rel-7): `Residence::detect` writes a probe with a new name
  below `%LOCALAPPDATA%`, lists `Packages` once and finds the container whose
  `LocalCache\Local` holds it, so a `timeout` or a pipeline between the app and the install no
  longer hides the container; the parent-chain walk is gone from `win-peer`. Checked from this
  batch's Claude Code session on the owner's machine: the probe lands in `Claude_pzs8sxrjxfjjc`,
  and `HKCU` and `HKEY_USERS\<sid>` both show the stale 1.x value naming a missing
  `%LOCALAPPDATA%\Ghostlight\NativeMessagingHosts\org.sylin.ghostlight.json` while the browser
  works. No in-process read reaches the real `HKCU` (the private hive is locked), so the registry
  view is private exactly when the probe lands privately (ADR-0197 amendment of 2026-10-18).
  Inside a container each browser row says "cannot be checked from here -- Claude's own view of
  the registry; run ghostlight doctor from a regular terminal." in `doctor` and the install dry
  run, and `doctor --json` gives those rows `not_visible` with the same words. A Ghostlight value
  naming a missing `org.sylin.ghostlight.json` is Missing, so install rewrites it; any other
  dangling value still needs attention. The writer golden did not change. Review fix: that rule
  holds only outside a container. Inside one the dangling value is the app's private copy, so
  calling it Missing made every in-container install list the browsers as kept private and send
  the person to a regular terminal whose install found nothing to do. Now the row stays
  unjudged there: the install writes no registry value, keeps none private, does not fail on it,
  and names it with the same "cannot be checked from here" line.
- a4 (install-8, docs-7, install-6, rel-8, trust-6, rel-12): on Linux an install whose
  `~/.local/bin` is not on the PATH it ran with says so in one summary line with the link's full
  path (`CommandPathActionResult::off_path`, worded in `language/environment.rs`); a new terminal
  is not offered there, because it would not be true: shells add that directory at login, if at
  all. Windows keeps "Open a new terminal" for a `Path` it just wrote. The README, the installation
  guide and `llms-install.md` lead recovery with `ghostlight doctor` and keep `npx -y ghostlight
  doctor` for npm installs. A service inside an app container leaves browser startup to the person
  the way the no-display case does: the launch starts nothing and ends as the manual answer, and
  `starts_on_request` names no browser there, so the window does not promise a start
  (`Residence::detect`, ADR-0149 amendment of 2026-10-18). `finish_setup` scans the browser
  profiles only while setup is incomplete, which is what docs/trust/data-flows.md says. Doctor
  lists `ghostlight-desktop` among its binaries; a missing window program does not mark the
  commands broken. The agent-config writer removes its `.ghostlight-<uuid>.tmp` on every failure,
  and a real install (not `--no-clients`) removes such files older than a minute beside the
  configurations it can change and its location state; a younger one may be a write in flight.
- f1 (j-23, j-32, j-33, j-35, j-36): the popup's Paused shows a held lantern (the waiting ghost
  with a still amber ember) and an amber title, and Session ended shows the waiting lantern, so
  neither looks like Ready. The shortcut cap on the teal Resume button takes the button's dark ink
  (6.4:1), the label stays on one line, and the cap drops below 260px. A quiet "Open Ghostlight"
  link in the popup and on the options page sends `show_ghostlight` while connected. Captions no
  longer depend on effects. A code panel's command never wraps and Copy moves under it below
  1000px. The setup headline balances and its accent never breaks. The options tab title lost its
  double hyphen, the only typewriter dash in visible extension copy. The window serves the
  extension's own `kit.css` bytes (`workbench/web/assets.rs`), so the code panel and headline fixes
  reach its setup view with no second copy to change. The popup foot's Settings link takes the same
  quiet ink as the new link beside it, so the foot rests in ink. `tests/pages.test.js` runs the
  popup and options scripts over a fake document. The freeze golden is regenerated from the new zip.
- c2 (j-02, j-10, j-06, j-13, j-12, j-22; window-4): the window adds `--ink-4: #8a8a93`, the
  kit's value, and sets every quiet word in it: inactive tabs, activity sentences, band facts, page
  leads, durations, ages and the settled stopwatch (now weight 400). `--muted` and `--faint` stay for
  strokes, borders and decoration, and a test fails on any `color` set in either in `styles.css` or
  `kit/kit.css`. The read capability keeps its #52525b edge and its word reads in ink-3 (`--cap-ink`).
  Two label voices remain: an eyebrow (500 11.5px sans, .18em, capitals, ink-4) for `.kicker`, the
  queue head, the rail's group heads and About's labels, and a mono data tag (500 10px, .14em) for
  capability, outcome, severity and rule tags; teal eyebrows stay in the setup view. Every
  destination's title is 300 25px in ink. No word is under 10.5px except a 10px tag; the sylin.org
  card and the face monogram are artwork. Policy's list is `.layer-settings`, so Settings starts
  under its heading and the restrictions are a plain list; a test checks the view draws none of the
  kit's layout containers. Every disclosure is a quiet 12px ink-3 line with the agents fold's
  chevron and turns to ink when open; `--sky` is gone from `ui/` and a test keeps it gone. The
  50px mascot is averaged rather than pixelated. The About orbs take their green and teal from
  classes, and a test finds no inline `style` anywhere in `ui/`.
- c3 (j-08, window-1, window-2, j-29, j-07, j-15, window-5, window-6, j-18): `ConnectionCounts` gains
  `agents`, one per agent id the way At a glance groups sessions, and `service_line` counts agents
  and browsers: "Ghostlight is running; 3 agents and 1 browser connected." (and "Ghostlight is not
  running."). The band facts, the pill, About ("Agents"), Status, the tray tooltip and `status` all
  read it; `status --json` and `doctor --json` keep `mcp_clients` and `command_line` beside it. The
  band's wheel, the tray and Settings say Pause agents and Resume agents. Status's kicker is "Is
  Ghostlight working" and its checks are Ghostlight ("Running and accepting agents."), Browser
  ("Ghostlight in Browser is connected in Google Chrome.", "Waiting for Ghostlight in Browser.", or
  the browser that starts on request), Rules ("No rules apply." or "Your rules are valid.") and
  History, all from `language/workbench.rs`. `readiness::resolve` ranks Session ended and Paused
  first; with no browser it is Not connected only when none would start, and otherwise Ready with
  "Starts Google Chrome when an agent asks." (the browser's own name, not always Chrome). Policy's
  open headline is "No rules apply. Agents may work on ordinary websites, within the limits below.";
  the first-rule-decides note shows only once a rule exists, and the refuses-everything warning only
  after a draft's rules were all removed. A draft that starts with no rules carries the open rule
  (`*`, all four capabilities) while it has none, says so once it changes, and a toggle alone
  applies. The Status Rules check says "No rules apply." rather than the spec's "no policy applied",
  so Status and Policy name the open case the same way. The service never learns the browser's
  Preserve Ghostlight tabs value (the extension is frozen), so the line beside Closing tabs says how
  it starts rather than claiming it is on, with a "Set in the browser" chip; the interlock refusal
  now names Ghostlight in Browser settings. The desktop goldens are regenerated and read.
- c4 (j-09, j-17, j-14, j-19, j-20): the activity list names one capability per row, the most
  consequential the action used (READ, ACTION, WRITE or EXECUTE, nowrap). The row's button carries
  the whole set for a screen reader (", read and write"), and the detail card's tag shows it
  ("read + write"). A flow carries no capability of its own (the service's empty set, "local"), so
  its row, edge and card take the union of its steps' receipts: one that clicked, typed and filled
  a form reads WRITE on the write edge, its card says "read + action + write" and "Steps: look at
  pages, click and type, fill in forms", and each step keeps its own label. A flow still running
  names none. Every width rule now asks the window's `shell` container (`.app-shell`) instead of
  the viewport, so Large text, which draws the shell larger, moves the layout with the room that is
  left; the setup view's one viewport rule stays, paired with the frozen extension kit. Below
  1040px the row drops the browser and the agent and keeps the capability and the age; below
  820px the capability moves under the tool's name. The band's facts hide below 1000px; below
  900px the tabs take a line of their own, so the state word and its chip never run under the
  wheel. Integrations stacks its heading's actions under the headline below 660px. Settings >
  This screen adds Text size (Standard, Large); Large sets `zoom: 1.15` on the shell, the setup
  view, the palette, the dialog and the notices, is kept with density and motion in the browser,
  and reads in the rail ("Comfortable, standard text, motion follows the system"). `html, body`
  hold at 600px. The window minimum the page is built for is 720 wide: lane native owns
  `WINDOW_MIN_SIZE` and set it to 720 x 620 in d1, so nothing changes there. Checked in a browser
  over the preview server at 1280, 960, 720 and 600 px, Standard and Large, on every destination:
  nothing runs past the shell and no page scrolls sideways. The stylesheet tests read container
  rules and check the row at 1040, 821, 820, 720 and 600, the band at 1001, 1000, 901 and 900, and
  the activity's room at 600.
- b3 (owner, 2026-10-18: "an MCP connector that's invoked by claude-code.exe has a clear hint"):
  the relay's listener keeps the peer process id its same-user check reads
  (`Connection::peer_process_id`), and `os::ancestor_images` walks at most 16 ancestors (a process
  snapshot and query-only opens on Windows, `/proc` on Linux), ending at an exited, unreadable or
  reused parent. `install::apps` finds each catalog agent's app where it is installed on every look
  (a Store package's install folders, a program's folder, a command or desktop-entry program file)
  and names the wrappers in one place (shells, consoles, interpreters, runners, `timeout`,
  Ghostlight's own executables), which the icon resolver now shares. `provenance::Source`
  (`agent`, `image` file name) is decided once when an `mcp` stream opens and joins the durable
  attribution, so audit lines carry `provenance.source` (goldens regenerated, SIEM guide updated)
  and lines without it read as before. `language::provenance` composes the words: "Started by
  Claude Desktop." when the source is the claimed agent's app, "Claude Code, in Claude Desktop."
  inside another agent's app, both without judging when they do not line up, the old caveat with
  no source, and "A ghostlight call from WindowsTerminal.exe." for the command line. No window
  part gains a key. Checked live on this machine: a cargo test process sees its real ancestors;
  the Git `timeout.exe` wrapper an agent's shell uses ends the readable chain, which is why the
  MCP edge, a direct child of its client, is the case that matters. Not checked: the edges under
  the owner's Store Claude Desktop, which needs the deployed build.
- e1 (rel-1, docs-1, trust-1, trust-2, trust-5, trust-8, docs-3, docs-10, docs-4, install-4,
  install-5, trust-4, docs-6, rel-10, f1 review): the CHANGELOG's tab-group line follows ADR-0201
  and the [2.0.0] section adds what a person notices: the PATH entry, agent icons, saved groups,
  watching on 127.0.0.1 and Settings > Sharing, the Store app settings copy, uninstall's summary,
  the 1.x leftovers, the connection's app, a call with no tab among several, page refusals, the
  window's and the popup's changes. "Observed peer identity" left the Removed list, since b3 brings
  the observed source back. `notes-v2.0.0-rc.1.md` is a prerelease preamble (the pinned commands,
  the unpacked extension, the group change, what 1.x users avoid) over the 2.0.0 notes. The man
  page's FILES names `~/.ghostlight` and the `~/.local` entries, documents `diagnostics` and drops
  `GHOSTLIGHT_RUNTIME_FILE`, which nothing reads; `governance::inspection::SUBCOMMANDS` is the
  policy list the completions and help are tested against. WinGet stays in the docs with one
  honest line: it carries 2.0 once the 2.0.0 manifest merges into winget-pkgs, which today holds
  1.3.6. RELEASE's cutover lists the steps before it (rc notes, the unpacked extension, the 1.x
  upgrade, the privacy page before the staged submission) and after the store publish (a
  clean-profile run, the public-state commit with a second fast-forward and a website rebuild,
  the MCP Registry, WinGet), and the store steps say the same.
