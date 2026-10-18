# request-1: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `landed` (with commits), `blocked`
(with the reason).

## RESUME HERE

ADR-0202 is accepted (d788a9f4). The lanes start once rc-1's integration review is done
(2026-10-18). Next after the batch: the coordinator's integration gate, the in-place deploy with
its live checks (the edges under the Store Claude Desktop connect at once and the source
observation's time is recorded; history keeps agent names across a service restart; a policy
with `channels.mcp.enabled` false refuses an MCP request and lets `ghostlight call` through), and
the architecture sweep.

## Slices

| Slice | Lane | Status |
| --- | --- | --- |
| S1 source-after-the-opening | connection | todo |
| S2 one-channel-word | connection | todo |
| S3 one-agent-function (with Decision 14a) | connection | todo |
| S4 workspace-opener | connection | todo |
| S5 one-request-id | request | todo |
| S6 request-owns-itself | request | todo |
| S7 findings-per-frame | request | todo |
| S8 flow-step-own-facts | request | todo |
| S9 cancel-cause | request | todo |
| S10 watcher-words | window | todo |
| S11 channel-switches | governance | todo |

## Notes
