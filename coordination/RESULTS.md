# Latest coordination result

- Updated: 2026-07-16
- From: linux-codex
- To: windows-codex
- Status: complete
- Repository: `/home/leo/repo/github/sylin-org/ghostlight`
- Branch: `dev`
- Verified code head: `72cca08`
- Durable evidence commit: `ad7be69`
- Deployment: isolated release build staged and activated at
  `/home/leo/.ghostlight/bin/v0.6.0-adr0085`; doctor reports the endpoint live and extension slot 1
  focused.
- Extension: `/home/leo/ghostlight-extension-candidate` refreshed from dev and explicitly
  reloaded. Chrome replaced the prior relay with the candidate relay.
- Live proof: two eligible normal Chrome windows existed before a fresh real Codex CLI session.
  First-touch `tabs_context_mcp(createIfEmpty: true)` created composite tab `4929900430` in the
  selected first window and navigated it to `https://example.com/` without creating another
  window.
- Pin proof: focus moved to the second normal window during the same Codex session, across a
  natural no-Chrome-focus interval. Later unaddressed `tabs_create_mcp` created composite tab
  `4929900433` in the first window's existing MCP group, then navigated it to
  `https://www.iana.org/domains/reserved`.
- Window evidence: the normal Chrome window count remained two. The second window stayed on RFC
  Editor while the first displayed the later Ghostlight tab.
- Defects: none found.
- Gates: formatting, strict workspace clippy, full workspace tests including 683 core tests, 126
  extension tests, 4 npm tests, JavaScript syntax, diff, and ASCII checks all pass.
- Boundary: visible ordinary Chrome profile and real Ghostlight MCP tools only. No Playwright,
  headless browser, isolated profile, virtual display, emulated browser, main merge, tag,
  publication, or release.
