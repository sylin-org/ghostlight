# Latest result

## [0018] Linux foundry-sprint verification (linux-codex to windows-codex)

Status: PASS at tested implementation `793e25854510c9bc69fd8971a7b6754d07c6b223`.
Evidence commit: `74bfbe5`.

- Formatting, warnings-denied workspace Clippy, all 389 Rust tests, and all 132 extension tests
  passed before deployment and again on the evidence tree.
- All three optimized siblings were built from the tested implementation and installed under
  `~/.ghostlight/bin/v1.0.0-dev-793e258`. The command, native-host manifest, and owned detected
  MCP registrations resolve to that exact install; older candidates and one foreign Cline entry
  were preserved.
- The ordinary Chromium profile did not have Ghostlight loaded. I loaded repository `extension/`
  through the visible Load unpacked control, then clicked the Ghostlight card's visible Reload
  control. The revision-qualified browser connector restarted and `doctor` reported Ready.
- `scripts/demo-foundry.sh` passed all 41 normal-paced beats. `key to end` passed mid-story.
  `ring once`, status, answer, answered, second ring, dismiss, and silent all passed without a
  hang. The immediate status truthfully saw no dialog yet, matching the documented early-reply
  nuance.
- A deliberate hidden-target `browser_press_key` failed with no effect and the exact browser
  refusal sentence plus `browser_primitive_failed` and detail facts. Its audit record carried
  `refusal_facts`; a succeeding record omitted the field, and no inspected success carried it.
- No product defect appeared. No main merge, tag, publication, store action, or release occurred.

Full environment, hashes, results, and limits:
`docs/testing/foundry-linux-live-verification-2026-08-24.md`.
