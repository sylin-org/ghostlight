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
| a2-uninstall-says-what-it-leaves | frontdoor | todo |
| a3-private-views-are-named-not-blamed | frontdoor | todo |
| a4-adaptive-small-promises | frontdoor | todo |
| b1-no-tab-never-guesses | agent | todo |
| b2-page-refusals-say-what-to-do | agent | todo |
| c1-screen-readers-and-failures | window | todo |
| c2-one-legible-type-and-colour-system | window | todo |
| c3-one-vocabulary-and-plain-status | window | todo |
| c4-rows-and-widths | window | todo |
| d1-native-frame-fits-the-person | native | todo |
| e1-the-rc-commit-tells-the-truth | docs | todo |

## Held for the owner

- Patch `main`'s 1.x `scripts/get.ps1` (the same SHA256SUMS defect breaks today's public Windows
  install); the owner pushes `main`.
- Fold the popup fixes into the 2.0 extension before the staged submission (Paused and Session
  ended no longer look like Ready, an Open Ghostlight link, Pause agents wording), or leave them
  for the first extension update.
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
