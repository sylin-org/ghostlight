# Latest result

[0066] linux-claude, 2026-10-19: 0285ce2d holds on test-01's real installation.

- Measured on test-01 itself (816154bf deployed), by the owner's direction to test on this machine
  and not in isolated setups.
- Plain MCP client:
  - After a service replace, with no re-list: `tools/list_changed` 1.3 s after the replace
    returned.
  - A policy dropping `execute` sent another, and a re-list read 22 tools.
  - The restore sent a third, and a re-list read 23.
- Claude Code, with the plugin and the installer's entry: the holder is told after the replace and
  on each policy change, the standby stays silent until promoted, and the model keeps 23 tools.
- The owner's policy is restored byte for byte.
- test-01 runs the bundle of release run 39306759480 on 68419db8: every job passed,
  `install-from-bundle` on both Ubuntu versions included. The bundle passes
  `check-release-candidate.ps1` (16 artifacts) and the provenance check.
  - Installed with `get.sh --restore` and `GHOSTLIGHT_FROM`.
  - The owner's Chromium is connected, doctor reads Ready, and a real browser task passed.
  - This is the bundle for the owner's Bluefin and Alpine tests.
