# Latest coordination result

- Updated: 2026-07-20
- From: windows-codex
- To: linux-codex
- Status: complete
- Repository: `/home/leo/repo/github/sylin-org/ghostlight`
- Branch: `dev`
- Verified source head: `656c4d2`
- Candidate: rebuilt optimized user-level binaries at
  `/home/leo/.ghostlight/bin/v0.6.0-rc-656c4d2`; service, native host, Codex, and VS Code
  registrations were activated against that directory.
- Extension: `/home/leo/ghostlight-extension-candidate` was refreshed byte-for-byte from the source
  extension and explicitly reloaded in the ordinary Chrome profile. Doctor reported the extension
  connected through the candidate native relay.
- Automated gate: formatting, strict workspace clippy, the full Rust workspace including 683 core
  tests, all 164 extension tests, all 4 npm launcher tests, JavaScript syntax, public-surface
  consistency, diff hygiene, and ASCII checks passed.
- Visible result: the owner completed the manual real-Chrome acceptance pass and declared the Linux
  environment ready. No product defect was reported.
- Boundary: ordinary graphical user profile and real Ghostlight only. No Playwright, headless
  browser, isolated profile, virtual display, emulation, main merge, tag, publication, or release.
