# The public 2.0.0 install and Tao 0.36.0 on CachyOS

test-01, the owner's CachyOS machine (KDE Plasma 6.8 on Wayland, one 1920x1080 output, system
Chromium 154), on its real installation and the owner's Chromium, 2026-10-20. Two checks that
windows-claude asked for after 2.0.0 was released.

## The public install command

Before: the installation was the release-channel 2.0.0 built from the candidate bundle of release
run 39306759480 (68419db8). Its edge and window images were byte-equal to the v2.0.0 release
assets; its service was not (installed `2aba7d3d...`, v2.0.0 `381b7aa8...`).

| Step | Result |
| --- | --- |
| `curl -fsSL https://sylin.org/ghostlight/get.sh \| sh` | The short URL serves `get.sh` from `main`. It printed "Ghostlight 2.0.0 downloaded. Checksums and build provenance verified." and "Ghostlight 2.0.0 is installed.", but kept the candidate's service: an intact install of the same version counts as current, whichever build it is. |
| The restore route, `sh -s -- --restore` | Installed the v2.0.0 service. All three installed images then equal the v2.0.0 `SHA256SUMS` entries. |
| `ghostlight doctor` | Ready. |
| `npx -y ghostlight@2 --version` | `ghostlight 2.0.0`. |
| The extension | test-01's repository drive mounts after Chromium starts, so the unpacked development extension was gone after the reboot. The store's Ghostlight in Browser had updated itself to 2.0.0 while disabled since 1.x; the owner enabled it, and it connected. |
| An agent rings the bell | A fresh `claude -p --model haiku` (8 turns), told to ring the demo desk's bell, read the dialog's status and accept: the page read `the bell says: "hello from the desk"`. The agent noted it could not tell whether the default was used, which the prompt-default change of 2.0.1 addresses. |

Finding: a person who installed a 2.0.0 candidate, which is every test machine, keeps the
candidate's images when the public command runs; only `--restore` replaces them. The summary says
"Ghostlight 2.0.0 is installed." either way.

## Tao 0.36.0 on KDE Wayland

dev's tip 8df3143e (Tao 0.36.0 from crates.io, after ac0026df) deployed in place with
`scripts/dev-loop.ps1`. KWin draws the window's titlebar (the frame is 28 px taller than the client
area), so ADR-0120's GTK header bar does not apply here; its case was still run with real pointer
clicks from a virtual absolute pointer on `/dev/uinput`, each step read from KWin's window list.

| Step | Result |
| --- | --- |
| Maximize, then restore, twice | Each click took: 1920x1050 at 0,0, then back to 1180x788 at 370,131. |
| Minimize | Minimized. |
| Restore by the compositor, as the taskbar does | Shown and active. |
| Close | No window; `ghostlight status` reads `tray`. |
| The tray | StatusNotifierItem "Ghostlight", Active; its menu holds Open Ghostlight, Pause agents, Resume agents (disabled while running) and Quit Ghostlight; Open through the menu shows the window. |
| `tests/linux/installed/desktop-check.py` | All five phases pass; after its lost-service phase the owner's Chromium reconnected by itself. |

test-01 now runs the development build of 8df3143e with the store extension 2.0.0.
