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
| S2 one-channel-word | connection | landed (this commit) |
| S3 one-agent-function (with Decision 14a) | connection | landed (this commit) |
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
- S2: `relay::protocol` names the words as `MCP_CHANNEL` and `CLI_CHANNEL`, and
  `IntakeChannel::as_str` returns them, so the recorded name and the sent word are one word. The
  ADR's one test `the_edge_and_the_command_line_send_the_opening_bytes_they_sent_before` is two,
  one in each crate that builds an opening:
  `forward::tests::the_edge_sends_the_opening_bytes_it_sent_before` and
  `cli::client::tests::the_command_line_sends_the_opening_bytes_it_sent_before`. Each builder
  became a small function (`forward::opening`, `client::mcp_opening`) so the test reads its bytes.
  `McpSession::new` takes no channel and gains a `Default`. The service tests' opening helper sent
  `npm` as the edge's channel and now sends `MCP_CHANNEL`; `npm` stays as the unknown word in
  `an_opening_with_an_unknown_channel_is_an_mcp_client`. Added
  `an_opening_on_the_command_line_channel_is_the_command_line` beside the two MCP cases.
- S3: `language::provenance::details(attribution, reported)` is the one function, live and from
  history (`ConnectionEvidence::details` passes the claim, `Attribution::details` none), so
  `live_details` is gone with `Agent::of_evidence`. Its closed `Relation` is between the agent it
  names and the source, which also covers a connection with no claim: the source's catalog agent
  names it and the words are "Started by ...", as b3 said for history. A live claim the catalog
  does not know, or a reserved name, still names the agent (by that name, or Unknown agent); only
  no claim at all falls to the source. `claimed_agent` is set in `ConnectionEvidence::new`, from
  the bounded claim, on MCP connections only. A restored row quotes the claimed agent's catalog
  name where a live row quotes the client's words (`"Cline" is what the agent calls itself.` for
  `"cline" ...`), since the words are never kept; the agent and every other sentence are the same.
  `install::catalog_agent` (by catalog id) and `install::runs_inside` join `agent_called`;
  `source_name` now looks the source up by id. The audit golden's claim is `claude-code`, so the
  goldens show `claimed_agent`. `workbench::history`'s one-sentence test claims a name the catalog
  does not know, since a catalog claim now names its agent in history. `ui/lib/view.js`'s comment
  is true as written and is unchanged, so no page file changed. `SessionSummary.agent` and
  `connection_counts` still derive from the workspace's label until S4.
- S10: beyond `PAUSED_FROM_WATCHER`, the `RuntimeSource::Watcher` doc comment
  (`workbench/mod.rs`) said "A browser tab that only watches", the same over-claim, and now says
  what is true. The web test `a_pause_from_a_watching_screen_says_so_on_every_screen` is renamed
  for a watching page, and the fake service in `tests/workbench-surface.mjs` speaks the new words.
