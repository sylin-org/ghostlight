# Latest result

## [0032] linux-1: L05 passes after six fixes to the shell, the service and the window

linux-claude, on test-01 (KDE Plasma 6.8, Wayland), on the real installation, under the owner's
authority to fix what it finds. Details: [the ledger](../docs/tasks/linux-1/LEDGER.md), L05.

The window and the tray were driven as a person would (Applications entry, clicks through a
virtual pointer, the tray menu over D-Bus, KWin's window list, screenshots). All L05 checks and
the Linux items of m2-03 and m2-04 pass after these fixes, each deployed and checked:

- da28f0f1 `fix(desktop)`: on Linux tao's `WaitUntil` is a plain wait, so a shell with only its
  tray reconnected about 18 s after a lost service; presence launched a second shell and the
  window then opened by itself. The shell keeps its own clock and reconnects in about 0.5 s.
- 17ec7571 `fix(desktop)`: the shell's desktop opening says what it shows (`window` or `tray`
  in the relay channel; the relay doc, the relay-protocol-4 table and a golden say so), and the
  service believes it: status is true after a restart, and a tray newcomer that finds the place
  taken shows no window. A shell that says nothing is treated as before.
- 240fcdeb `fix(desktop)`: `ghostlight open` with no shell running gave a shell with no window
  that ignored every later open (a window replaced at birth is never reported destroyed by
  tao). The shell records its own window's end, and a person's start answers the service's
  follow-up show.
- f546c124 `fix(desktop)`: the tray hover reads Ghostlight, not `org.sylin.ghostlight`.
- 0a09d95b `fix(workbench)`: the band says "1 session" and "1 browser".
- 906bd254 `fix(status)`: did_not_stay names WebKitGTK as a check, not as the cause.

Tests: d86463ca aligns `tests/linux/installed/desktop-check.py` with the 2.0 shell on the real
installation; it passes. docs/MEMORY.md has the tao lesson.

Please pull before touching `crates/desktop` (app.rs, link.rs, window.rs), `service/presence.rs`,
`service/mod.rs` (serve_desktop) or `intake/desktop.rs`.

windows-claude's [0031] (integration of both sessions' work, the `Scratch` test guard, the
hand-moved idle clock, icon tiles, Store-app settings copies, and the Windows CI repair of the
retired-pages test) is in git history at 87903254. Its asks are taken: Unix-only test calls get
`#[cfg(unix)]`, and L09 looks at the Linux icons on their new tiles.

Next: L06, headless and SSH.
