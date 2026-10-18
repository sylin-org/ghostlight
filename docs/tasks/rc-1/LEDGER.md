# rc-1: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `landed` (with commits), `blocked`
(with the reason).

## RESUME HERE

The four lanes and the docs lane are running (2026-10-18). Next: the coordinator's integration
gate, the in-place deploy and the owner-held items below.

## Slices

| Slice | Lane | Status |
| --- | --- | --- |
| a1-get-ps1-reads-real-releases | frontdoor | landed (`fix(install): the Windows one-line installer reads a real release's checksums`) |
| a2-uninstall-says-what-it-leaves | frontdoor | landed (`feat(uninstall): uninstall says what it removed and what it keeps`, `fix(uninstall): a partial uninstall and its dry run say what it really removes`) |
| a3-private-views-are-named-not-blamed | frontdoor | todo |
| a4-adaptive-small-promises | frontdoor | todo |
| b1-no-tab-never-guesses | agent | landed: with several tabs a call that names none is refused and lists them; no per-caller marker exists (ADR-0198, amendment of 2026-10-18) |
| b2-page-refusals-say-what-to-do | agent | landed: a hidden, disabled, read-only or non-text control is refused in plain words, says nothing was clicked or typed, and names one step that works (ADR-0198, amendment of 2026-10-18) |
| c1-screen-readers-and-failures | window | landed (`fix(workbench): the window is calm for a screen reader, and a failure looks like one`): one status line says each settled action, at most one a second; failed and kept rows and the card say so in words; a destination that cannot be read says so in place, with Try again; both dialogs hold focus and give it back |
| c2-one-legible-type-and-colour-system | window | landed (`fix(workbench): one legible type and colour system on every tab`): every word is at least --ink-4 (5.6:1) and about 11px; one eyebrow, one data tag and one page title; Policy's restrictions no longer collide with the kit's Settings grid; summaries, the agent pane's mascot and the About orbs fixed |
| c3-one-vocabulary-and-plain-status | window | landed (`fix(workbench): the window, the tray and status say the same thing in plain words`): one count, the agents At a glance groups, on the band, the pill, Status, About, the tray and `status`; Pause agents and Resume agents everywhere; Status says Ghostlight, Browser, Rules and History in plain words; a pause outranks no browser, and a browser that starts on request reads Ready; Policy says the open case once and a setting alone applies; the Windows first-close notice names the hidden icons and the Start menu; Closing tabs names Preserve Ghostlight tabs (ADR-0199 and ADR-0187 amendments of 2026-10-18) |
| c4-rows-and-widths | window | landed (`fix(workbench): the activity list keeps its meaning at every width, and the window fits a laptop half`): a row names one capability, the most consequential, and a flow takes its steps' (no more LOCAL); the band's facts give way below 1000px and the tabs take their own line below 900px; every destination holds at 720 and 600 CSS px; Settings > This screen offers Text size Standard / Large. The window minimum is 720 (lane native, `crates/desktop/src/window.rs` `WINDOW_MIN_SIZE`, already 720 x 620) |
| d1-native-frame-fits-the-person | native | landed (`feat(tray): the tooltip names the state and says when no browser is connected`, `feat(desktop): the frame and the tray fit a light-mode laptop`): the title bar is dark in light mode; a new window opens no taller than the primary monitor less 96 px (624 high on 1280x720), centered above the taskbar; the minimum width is 720; the Windows tray draws the 16 px ghost at 16 px and the 32 px one at 24 and 32, and Linux always draws the 32 px one for its host to size, inks it on a light taskbar (re-read every 2 s) and rings a 3 px dot in the taskbar's ground; the executable carries 16/32/48/128; the tooltip leads with the band's word and says "No browser connected" (ADR-0199 amendment of 2026-10-18). Owed: the light-mode 150% check under Before v2.0.0 |
| f1-extension-popup-and-kit | extension | landed (`fix(extension): the popup tells Paused apart and reaches the window`) |
| e1-the-rc-commit-tells-the-truth | docs | todo |

## Held for the owner

- Patch `main`'s 1.x `scripts/get.ps1` (the same SHA256SUMS defect breaks today's public Windows
  install); the owner pushes `main`.
- The popup fixes fold into the 2.0 extension (owner, 2026-10-18, "Proceed."): slice f1 and the
  ADR-0200 amendment of that day. The Pause agents wording is not part of f1.
- The accent rule: the coordinator applies "teal means live" (teal for work happening and the one
  primary action per view) unless the owner rules otherwise.
- Dispatch the "Build release candidate" workflow as a rehearsal (no tag, sign or publish).

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
