# Ghostlight 1.3.5

Ghostlight now keeps human browsing outside its control and makes browser work more reliable
across startup, upgrades, embedded documents and Linux desktop environments. Chrome adapter
1.1.4 is the matching public adapter for these changes.

## Fixed

- Human navigation, tab creation, tab movement and child popups no longer become Ghostlight
  activity or policy events. Stale agent references expire silently and the next actual agent
  request checks the current destination.
- Targeted typing remains bound to the intended document and reports the destination of blocked
  agent work.
- Concurrent desktop startup is bounded around one ready authority, failed owned children are
  reaped, and retained clients recover after executable replacement.
- Linux portable extraction and retained-client package upgrades are repaired. Windows installer
  native-host paths and deployment-lock ownership are repaired.
- JavaScript dialog-opening events are recorded before optional before-unload handling, so prompts
  remain available until answered or dismissed.
- Script and flow effects, document coverage, diagnostics, audit continuity and human controls stay
  truthful through reconnect and recovery paths.

## Changed

- Flatpak Chromium activation is available explicitly for home-resident installations on default
  data roots, using one named bus permission and owned registration/removal. Setup does not restart
  the user's browser.
- Owned Linux Codex configuration forwards live desktop-context variable names.
- Host and capability authority live in policy. Browser flow replaces the retired browser sequence
  tool and caller-supplied restrictions and dry-run are removed.
- At a glance shows grouped incremental action history with expandable details.

## Install

Use `npx -y ghostlight@1.3.5`, the NSIS installer, the Debian package, or the portable archives.
Install Chrome adapter 1.1.4 from the Chrome Web Store. Release artifacts are checksum-bound and
provenance-attested; see `SHA256SUMS` and `release-candidate.json` in the release assets.
