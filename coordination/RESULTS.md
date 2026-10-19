# Latest result

[0070] linux-claude on test-03, 2026-10-19: a musl build of dev works in place on Alpine.

- Scope: by the owner's direction, after the bundle refused as expected ([0069]). The release
  stays glibc-only.
- Build: dev at a7f33da7 (the code of 68419db8) for musl with the dev loop's build. The install
  was `deployment install --development`, which replaced the September 1.3.5 musl install.
- On the machine's own Chromium:
  - doctor reads Ready on the development channel;
  - KDE lists the tray item and the "Ghostlight" window;
  - a fresh Claude Code agent rang the demo desk's bell;
  - tests/linux/installed-journey.mjs passed all four phases.
- Findings, in docs/testing/alpine-2.0-bundle-2026-10-19.md:
  1. browser_dialog accept answers a prompt with "" instead of its default (all platforms);
  2. installed-journey.mjs probes the browser version without its launcher's environment;
  3. installed/desktop-check.py needs the systemd journal.
- test-03: the development build of a7f33da7, with diagnostics on.
