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
| S1 source-after-the-opening | connection | landed (this commit) |
| S2 one-channel-word | connection | todo |
| S3 one-agent-function (with Decision 14a) | connection | todo |
| S4 workspace-opener | connection | todo |
| S5 one-request-id | request | todo |
| S6 request-owns-itself | request | todo |
| S7 findings-per-frame | request | todo |
| S8 flow-step-own-facts | request | todo |
| S9 cancel-cause | request | todo |
| S10 watcher-words | window | landed (this commit) |
| S11 channel-switches | governance | todo |

## Notes

- S1: the source observation for a `cargo test` peer on this machine takes about 100 ms
  per read: two runs of 10 reads had medians of 107 ms and 103 ms, the slowest 128 ms. It is
  measured by the ignored test `provenance::tests::the_source_observation_time_on_this_machine`
  (`cargo test -p ghostlight-service source_observation_time -- --ignored --nocapture`).
- S1: `Attribution::observed` takes the reader beside the peer, `observed(peer, &SourceReader)`,
  so the test intake injects a held read; the service's `SourceReader::system()` is
  `Source::observe`. Tests keep the label-and-channel helper as the test-only
  `ConnectionEvidence::claimed`, so `work/tests/provenance.rs` changes only its two constructor
  names. `a_client_that_goes_away_badly_still_releases_its_session` now waits for its session to
  open, since a fresh session opens after its `initialize` answer. `HARNESS_ATTACHED` reads
  `<connection> via <channel> from <image>`, and without a source it is unchanged.
  `tests/provenance-journey.mjs` is left to the coordinator's live checks.
- S10: beyond `PAUSED_FROM_WATCHER`, the `RuntimeSource::Watcher` doc comment
  (`workbench/mod.rs`) said "A browser tab that only watches", the same over-claim, and now says
  what is true. The web test `a_pause_from_a_watching_screen_says_so_on_every_screen` is renamed
  for a watching page, and the fake service in `tests/workbench-surface.mjs` speaks the new words.
