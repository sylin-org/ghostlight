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
| S4 workspace-opener | connection | landed (this commit) |
| S5 one-request-id | request | landed (0f1b1a78 and this commit) |
| S6 request-owns-itself | request | landed (this commit) |
| S7 findings-per-frame | request | landed (this commit) |
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
  the bounded claim, on MCP connections only. Only a live client's words are ever quoted: a
  restored row keeps none, so where a live row says `"cline" is what the agent calls itself.
  Started by Claude Desktop.` the restored row says `Started by Claude Desktop.`, and with no
  source it keeps the MCP caveat. The agent and the Own and Inside sentences are the same.
  `tests/workbench-preview-server.mjs` names its agents by the same rule and shows a restored row
  named by its kept claim.
  `install::catalog_agent` (by catalog id) and `install::runs_inside` join `agent_called`;
  `source_name` now looks the source up by id. The audit golden's claim is `claude-code`, so the
  goldens show `claimed_agent`. `workbench::history`'s one-sentence test claims a name the catalog
  does not know, since a catalog claim now names its agent in history. `ui/lib/view.js`'s comment
  is true as written and is unchanged, so no page file changed. `SessionSummary.agent` and
  `connection_counts` still derive from the workspace's label until S4.
- S4: `WorkspaceStore::client_label` is `opener_claim`, and the pool's `PooledTab.client_label`
  and `claim_released`'s parameter are named for the opener's claim too, so the tab-group title
  and released-tab reuse read as the rule; their behavior is unchanged. `WorkspaceSummary` also
  lost `leased` and `held_tab_count`: once `SessionSummary` dropped them nothing read them. The one
  test that read `held_tab_count` already proves the tab is not held, since its `select_tab`
  refuses a held tab. `SessionSummary.channel` stays and is the opener's channel. The preview
  server's fixture sessions lost the two retired keys. The two session-card tests are in
  `workbench::tests` and the rename test is in `workspace::tests`, where the group title's claim is
  read; `work/tests/tabs.rs` changes only the method's name.
- S5: `InvocationId` is `pub`, not `pub(crate)`, because `ServiceRequest`, `ServiceResponse` and
  `InvocationResult` are public. The id is typed through `PreparedInvocation.id`,
  `InvocationContext.invocation` and `InvocationResult.invocation`, which serializes as its text,
  so the envelope and every golden are unchanged; events, audit and the window keep the text, and
  the executor's tables still key by text until S7. The two commits are the id, then the caller
  deadline: `ServiceRequest::Invoke.deadline_ms`, the branch, its assertion and the parameter of
  `PreparedInvocation::new` and the test-only `execute` (325 call sites) go together, so no commit
  holds an unused parameter. `a_cancel_reaches_the_invocation_the_client_named` is in
  `service::admission::tests`: a session's forwarded call and cancel drive the session queue,
  since the fake browser cannot hold a call open over a relay pair. There was no session
  duplicate-id test to rekey, so `a_running_client_id_is_refused_and_is_free_again_once_answered`
  is new; `string_and_numeric_ids_echo_with_their_type` no longer checks the prefix, which
  `invocation_id::tests` does. `work/tests/provenance.rs` and `work/tests/control.rs` pass the id
  to `PreparedInvocation::new`. The admission duplicate test builds a second job under the same
  id until S6 deletes `Refused::Duplicate`. The ARCHITECTURE Work paragraph rode in with S3's
  commit, since both lanes had the file open. LANGUAGE and a comment in `language::outcome` no
  longer mention an edge deadline.
- S6: `InvocationContext.provenance` and `Completion.provenance` became required references too,
  since every request now has its evidence; `AuditRecorder::record_with_provenance` and
  `AuditRecord::with_provenance` keep their `Option` for records that are not requests, and the
  completion passes `Some`. `OperationState` holds the required details, while
  `OperationSummary.provenance` keeps its `Option` type, so the window's JSON and the parts tests
  are unchanged (every live row now has one). The test-only `execute` gives the request the
  caller's token, so its 325 call sites are unchanged. The queue's `jobs()` is `admitted()`, and
  `SESSION_WORK` is `pub(super)` for the full-queue test. The duplicate admission test became
  `a_queued_cancellation_bypasses_work_and_touches_no_later_request`. The full-queue and disconnect
  tests are service tests over a relay pair: the test holds the lease of a declared session's
  workspace, so calls wait in its queue. `a_waiting_call_keeps_its_start_time_when_it_runs` is in
  `work::tests::control`. Two tests used `duplicate_request` as a sample service code and now use
  `SHUTTING_DOWN_CODE`, the one code the service still sends. ARCHITECTURE's domain event list
  now names `WorkWaiting` and `WorkPhaseStarted`; its `WorkProgressed` never existed. The S6
  row and this note first landed in 80868c31, which another lane committed while S6 was still in
  the working tree; the code landed in the commit that adds this sentence.
- S7: a frame's stack value is `work::findings::Frame`, its settlement budget and its
  `FrameFindings`, the `RefCell<Findings>` behind short methods (`observe`, `reach`, `permit`,
  `cover`, `offer`, `take_offer`, `take`), so `InvocationContext` gains one `findings` reference
  and every `..*context` override keeps its frame's findings. The constructors are
  `InvocationContext::of(prepared, operation, snapshot, frame)` and `context.step(operation,
  frame)`. `retain_permission` went with its table, and each check writes
  `context.findings.permit`; `retain_coverage` stays as the one place that merges a frame's
  coverage and sends the live notice. The remaining hand-built `Completion` is the undecoded
  request's, which has no frame. The operation lines read `<tool> <status> <effect> <ms>ms on
  <connection>`, through `receipt::operation_line`, which the test reads. The new tests are in
  `work/tests/findings.rs`; the concurrent one holds a scoped read in a wrapping port and runs
  `policy_explain` beside it, and shares the documents suite's fixture helpers, now
  `pub(super)`. `an_observation_never_outlives_the_invocation_it_describes` went with the table
  it guarded.
- S10: beyond `PAUSED_FROM_WATCHER`, the `RuntimeSource::Watcher` doc comment
  (`workbench/mod.rs`) said "A browser tab that only watches", the same over-claim, and now says
  what is true. The web test `a_pause_from_a_watching_screen_says_so_on_every_screen` is renamed
  for a watching page, and the fake service in `tests/workbench-surface.mjs` speaks the new words.
