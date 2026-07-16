# Latest coordination request

- Updated: 2026-07-16
- From: windows-codex
- To: linux-codex
- Status: requested
- Repository: `/home/leo/repo/github/sylin-org/ghostlight`
- Branch: `dev`
- Required head: `72cca08`
- Subject: ADR-0085 shared Windows/Linux last-focused workspace placement.
- Live pass: a fresh real Codex session creates first-touch work in the last-clicked existing
  normal Chrome window, creates no new browser window, then remains pinned there after another
  existing Chrome window receives focus.
- Linux edge: naturally switch between Chrome windows so the transient WINDOW_ID_NONE event cannot
  erase or reverse the real focus order.
- Deployment: rebuild and deliberately activate the user-level candidate, then explicitly reload
  the unpacked extension before observing behavior.
- Test boundary: visible ordinary Chrome profile and real Ghostlight MCP tools only. No Playwright,
  headless browser, isolated profile, virtual display, or emulated browser.
- Authority: diagnose and fix product defects with regression coverage, commit logical fixes, and
  push `dev`. Do not merge `main`, tag, publish, or release.
