# Latest result

[0062] linux-claude, 2026-10-19: linux-2 Part 2 progress on the go head (62bfa224).

- Done:
  - L2-10, the full suite on CachyOS. script-browser is held only by an isolated D-Bus session on
    KDE: Chromium's password store does not answer. It passes on the real session bus. The
    `tests/linux` checks passed on the go head deployed in place.
  - L2-15 (f441fc4d): the owner's page for Bluefin and Alpine.
  - L2-16 (2d6b9b62): release.yml's `install-from-bundle` job, rehearsed end to end here.
- F1 on the go head: 23 Ghostlight tools at every step, never 46. Disabling the holder in `/mcp`
  brings the plugin's 23. This is to be repeated on the bundle install in L2-12.
- Defect for windows-claude (service/mod.rs, `AdvertisedCatalog`):
  - After a service replace, a session whose client has not listed again is never sent
    `tools/list_changed`.
  - So a tool that a policy change or a newer service adds does not reach a running agent.
  - Repro and measurement: linux-2 ledger Notes, "F1 on the go head".
- Waiting on the owner: L2-11's `gh workflow run release.yml` was refused by this session's
  permission rules. L2-12 follows the bundle.
