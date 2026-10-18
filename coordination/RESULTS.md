# Latest result

[0052] linux-claude, 2026-10-18: the torch goes back to windows-claude; CI is green.

- cf056c5f `test(recovery)`: CI rust (ubuntu-latest) had failed on every dev run since cb37cdbf
  in `no_browser_is_launched_while_the_session_is_ending`. Since cb37cdbf a launch that finds no
  display ends as NoDisplay, so the test passed only on a host with a display. Its browser now
  has no executable, so the attempt fails before the display probe on every host. Test only.
- CI run 39135419237 on cf056c5f: all 11 jobs green, Linux and Windows.
- Landed on 2026-10-18 since [0046], all pushed:
  - 0018941b (ADR-0201): saved tab groups no longer pile up. `extension/lib/tabs.js` changed,
    and the freeze golden is regenerated (ADR-0200 amendment of 2026-10-18).
  - f1096586: browser_navigate's `browser` field says it needs `new_tab` true.
  - 7abbdc39: LANGUAGE.md states the catalog ceiling as 31,800.
- Windows pickup:
  - Pull dev.
  - Redeploy with `scripts/dev-loop.ps1`: the service gained `BrowserCommand::AdoptTab`.
  - The owner reloads the unpacked extension in `chrome://extensions`.
- Open for Windows: the 1.x window data under `%LOCALAPPDATA%\org.sylin.ghostlight` is not
  removed there yet (`migration::leftover_directories`).
- Owed on Linux: the pinned install of `v2.0.0-rc.1`, once it exists (linux-1 ledger, L10).
- test-01 runs the development build at 7abbdc39, with the owner's Chromium connected.
