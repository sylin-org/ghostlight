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
| a2-uninstall-says-what-it-leaves | frontdoor | landed (`feat(uninstall): uninstall says what it removed and what it keeps`) |
| a3-private-views-are-named-not-blamed | frontdoor | todo |
| a4-adaptive-small-promises | frontdoor | todo |
| b1-no-tab-never-guesses | agent | landed: with several tabs a call that names none is refused and lists them; no per-caller marker exists (ADR-0198, amendment of 2026-10-18) |
| b2-page-refusals-say-what-to-do | agent | todo |
| c1-screen-readers-and-failures | window | landed (`fix(workbench): the window is calm for a screen reader, and a failure looks like one`): one status line says each settled action, at most one a second; failed and kept rows and the card say so in words; a destination that cannot be read says so in place, with Try again; both dialogs hold focus and give it back |
| c2-one-legible-type-and-colour-system | window | todo |
| c3-one-vocabulary-and-plain-status | window | todo |
| c4-rows-and-widths | window | todo |
| d1-native-frame-fits-the-person | native | landed (`feat(tray): the tooltip names the state and says when no browser is connected`, `feat(desktop): the frame and the tray fit a light-mode laptop`): the title bar is dark in light mode; a new window opens no taller than the primary monitor less 96 px (624 high on 1280x720), centered above the taskbar; the minimum width is 720; the tray draws the 16 px ghost at 16 px and the 32 px one at 24 and 32, inks it on a light taskbar (re-read every 2 s) and rings a 3 px dot in the taskbar's ground; the executable carries 16/32/48/128; the tooltip leads with the band's word and says "No browser connected". Owed: the light-mode 150% check under Before v2.0.0 |
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
