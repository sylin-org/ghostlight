# Latest result

[0064] linux-claude, 2026-10-19: linux-2 is done on CachyOS (with the [0064] corrections).

- Done:
  - L2-02, L2-04, L2-10, L2-15 and L2-16.
  - L2-11: release run 39297671342 on 149c04d6 passed every job, `install-from-bundle` on
    ubuntu-22.04 and 24.04 included. Its bundle passes `check-release-candidate.ps1` (2.0.0, 16
    artifacts), with provenance.
  - L2-12 on test-01 with that bundle:
    - Clean uninstall of the development install, then the real `get.sh` with `GHOSTLIGHT_FROM`.
    - The owner's Chromium connected, and doctor read Ready.
    - A fresh Claude Code agent completed the demo desk task.
    - The window and tray checks passed, F1 held, and idle exit came after 632 s (measured in a scratch home only).
    - A browser restart mid-session on the owner's Chromium: the extension reconnected within 1 s,
      both handles read `tab_unavailable`, and the agent reopened the page.
    - A second uninstall and reinstall were clean.
- Blocked by the owner's CachyOS-only decision: L2-01, L2-03, L2-13 and L2-14.
- Open for windows-claude:
  - The `AdvertisedCatalog` defect: a resumed session misses `list_changed` (ledger Notes).
  - From L2-12: `browser_tabs list` succeeds with no browser connected, against its description.
- test-01: the bundle (release 2.0.0) with the owner's policy and diagnostics on. The previous
  home is at `~/.ghostlight.l212-before`.
