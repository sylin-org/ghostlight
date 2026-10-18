# Latest result

## [0044] linux-1 is done: L01 to L10 pass on test-01, with the fixes they needed

linux-claude, on test-01 (CachyOS, KDE Plasma 6.8 on Wayland), on the real installation and the
system Chromium. The record is [the ledger](../docs/tasks/linux-1/LEDGER.md); each fix there was
deployed and checked live.

Since [0040]:

- 5107be98 `fix(workbench)` (L09): an app icon whose plate sits in a transparent margin, like
  Zed's, is its own plate and is served as cached (no more dark plate inside an off-white square).
- e964e858 `fix(setup)` (L10): a whole `ghostlight uninstall` asks the running service to quit with
  `uninstall` and says so; until now the service, the tray shell and the browser relay kept
  running after it and nothing sent that goodbye. The home stays (ADR-0199 Decision 20).
- 1024a994 `fix(workbench)` (L08): at the setup view's agent step a long agent list no longer
  pushes the mascot and the steps below the fold; the window's `styles.css` keeps the companion
  centered on the window (the extension kit is unchanged). This one was not claimed first; it
  touched only that stylesheet.

For the owner, not changed (details in the ledger, "seen and left"):

- L08: when the extension is in a profile but no browser is connected, the install's Next line
  says "restart your agents, then ask one" while the window shows the browser step.
- L07: on plain Chromium the extension names no browser, so no ADR-0149 last-browser record is
  written; an upload calls a file input a "text field".
- L10: 0.x and 1.x left `~/.local/share/ghostlight`, `~/.local/share/org.sylin.ghostlight` and an
  emptied `~/.local/state/ghostlight`.

Machine: test-01 runs the development build at 1024a994 in a fresh `~/.ghostlight` with the
owner's policy copied back; the home from before L10 is kept at `~/.ghostlight.l10-before`.

Still owed: the release candidate's pinned install on Linux (`get.sh` with `GHOSTLIGHT_VERSION`),
once `v2.0.0-rc.1` is published.
