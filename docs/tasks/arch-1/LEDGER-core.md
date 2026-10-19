# arch-1: core ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

C1, C2, C3a, C4, C5, C6, C7, C13, C9, C10a, C10b, C11, C12a, C19a and C21 have landed, and C22
landed with C13. Every candidate slice of core is in; B1 and C21 ride in the same candidate. Next
are the slices gated none: C3b, C3c, C12b, C10c (after P1 and P5), C16, C17, C18, C19b, C19c and
C20 (after I9b).

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| C1 | fix(audit): the durable record keeps only what may be retained | candidate | ux-2 closes | see Notes | landed (fe18e597) |
| C2 | fix(workspace): a tab binding has one lifecycle | candidate | ux-2 closes | see Notes | landed (7bd0bf20) |
| C3a | fix(work): a session's browser is bound by its first tab | candidate | C2 | see Notes | landed (ec15bacb, journey fixup) |
| C4 | fix(workspace): a remembered tab is pending until its browser confirms it | candidate | C3a | two commits; see Notes | landed (e4f353ab, 1d10bf74, review fixup) |
| C5 | fix(service): a service end decides its sessions' tabs once | candidate | C4 | see Notes | landed (9beb7976) |
| C6 | fix(language): a held tab is described truthfully | candidate | C2 | see Notes | landed (5122ae5a, review fixup) |
| C7 | fix(language): a workspace refusal says what is true | candidate | C6 | see Notes | landed (8f2241c0, e2d6b413) |
| C13 | fix(provenance): the opener is named once | candidate | C7 | see Notes | landed (c78d4bd3) |
| C9 | fix(work): a duration wait says what its deadline did | candidate | A1, A2 | see Notes | landed (565293f6) |
| C10a | fix(work): closing and listing a tab look at its host | candidate | C2 | see Notes | landed (51d1e05d, review fixup) |
| C10b | fix(work): every authority check of a frame is on the frame's path | candidate | ux-2 closes | see Notes | landed (56d0f7c8) |
| C11 | fix(service): a call during a quit and a token that cannot be read both get an honest answer | candidate | ux-2 closes | see Notes | landed (8c216920) |
| C12a | feat(language): history rows carry one closed outcome class | candidate | ux-2 closes | see Notes | landed (a35d7b58) |
| C19a | fix(audit): a step that never started carries the policy sequence | candidate | ux-2 closes | see Notes | landed (647b4d97) |
| C21 | feat(adapter): the service reads the browser's life | candidate | C4, B1 | lands with B1; cross-lane `protocol4/wire.rs`; see Notes | landed (this commit) |
| C3b | fix(browser): a pinned wait reads its own browser | none | C3a | - | todo |
| C3c | refactor(browser): one browser start, recorded in the launch guard | none | C3b | - | todo |
| C12b | refactor(language): each refusal is typed where it is learned, and its code is written once | none | ux-2 closes | two commits | todo |
| C10c | refactor(work): the executor reads one publication | none | P1, P5 | - | todo |
| C16 | refactor(work): which recording a workspace holds is a workspace fact | none | ux-2 closes | cross-lane `protocol4/link.rs`, `workbench/tests.rs` | todo |
| C17 | refactor(language): one wait vocabulary and one coverage predicate | none | ux-2 closes | cross-lane `translate/observe.rs` | todo |
| C18 | refactor(presentation): one table of activity and denial words, with the crossing names pinned | none | C6 | cross-lane `protocol4/golden.rs` and the page assets | todo |
| C19b | feat(audit): the record names the browser | none | I1 | - | todo |
| C19c | refactor(work): the completion path's leftovers go | none | ux-2 closes | - | todo |
| C20 | test(language): the plugin skill names only catalog tools and fields | none | I9b | - | todo |
| C22 | refactor(intake): the "MCP client" placeholder goes | none | C13 | cross-lane `agent/mod.rs`, `workbench/tests.rs`; see Notes | landed (with C13, c78d4bd3) |

## Notes

- C1, the four durable sentences: `Refusal::audit` renders from `retained()`, and four refusals
  keep a durable sentence of their own because their own sentence is made of what history never
  keeps: the browser's words (`BrowserPrimitive`), the fields a fill could not set
  (`FieldValuesRejected`, `FieldsReverted`) and the credential control. These are sentences, not
  strip arms, and they read the retained value. A refused or undone fill keeps its field list in
  the retained value only so its sentence can count it.
- C1, beyond the ADR's list: `retained()` also clears the browser's text from `BrowserPrimitive`
  and the credential control's handle, not only its name. `WorkspaceReason::retained` is its own
  exhaustive match, so C7's new causes decide there what the audit keeps. Every `retained()` match
  names each variant, so a new variant cannot skip the decision.
- C1, 1.3.14: the fill read-backs and the dialog message did not exist in 1.3.14, so that leak is
  2.0-only. The ownership leak is in 1.3.14: an ownership refusal wrote the other workspace's
  client label, or its workspace id when it had none, into the audit summary and into
  `refusal_facts`. D2's correction paragraph in `data-flows.md` applies to it.
- C1, proofs: `no_page_text_reaches_the_audit_through_any_wrapper`,
  `an_ownership_refusal_writes_no_other_clients_name` and
  `the_retained_refusal_drops_the_control_page_address_the_credential_name_and_the_browsers_words`
  in `language/audit.rs`; `a_fill_whose_submit_raises_a_confirm_writes_no_field_value` in
  `work/tests/execution.rs`. No audit golden changed: none of them carries a wrapper or an
  ownership refusal. No journey is needed.
- C2, the held tab: the lease has `select_tab_to_leave`, which selects a held tab, beside
  `select_tab`, which refuses it. Only `navigate_page` on an existing tab and `close_tab` call it.
  A navigation that names no tab in a session whose only tab is held selects that tab too, because
  it is an existing tab. The close of a held tab is still authorized as every close is today
  (Action with no host, then the tab-close setting); C10a narrows it to the setting alone.
- C2, `restore_tab`: it no longer carries a hold over. A hold was carried only when the handle was
  still bound, and a binding at generation 0 now clears whatever the handle carried, its hold
  included.
- C2, `WorkspaceSummary::of` is private to the aggregate. The window still reads `browser_of` for
  each workspace; S3 moves it onto `WorkspaceSummary.browser`.
- C2, proofs: `a_tab_the_person_closes_leaves_no_snapshot_or_image` and
  `a_recreated_handle_never_serves_the_closed_pages_capture_or_diff` in `workspace/mod.rs`;
  `a_held_tab_can_be_navigated_to_an_allowed_page_and_is_released`, `a_held_tab_can_be_closed`
  and `a_held_tab_still_refuses_reads_actions_history_and_focus` in `work/tests/tabs.rs`;
  `a_read_that_holds_a_tab_announces_the_hold` in `work/tests/presentation.rs`. No golden changed
  and no journey is needed. The hold words stay as they are until C6.
- C3a, one frame, one browser: until a tab binds the session, a frame keeps its own first browser
  choice (`FrameFindings::target`), so one call never crosses to two browsers. An open chooses its
  browser before it sends `OpenTab`, and the tab it opens binds that browser through `add_tab` or
  `restore_tab`, which now take the browser. The ADR does not name this; without it an open could
  bind one browser while its tab opened in another when the attended browser changed mid-call.
- C3a, `pin_browser` goes: the only writers of a workspace's browser are `add_tab` and
  `restore_tab`, which refuse a tab of another browser (`BrowserPinned`). Tests that needed a
  pinned session bind a tab instead. Cross-lane: one `add_tab` call site in surface's
  `workbench/tests.rs`, a signature change only.
- C3a, the resume path: `restore_tabs` no longer pins before it restores. A resume that confirms no
  tab binds no browser. C4 rewrites this path.
- C3a, proofs: `listing_tabs_in_an_empty_session_binds_no_browser`,
  `an_empty_session_lists_at_once_without_a_browser`,
  `an_empty_session_opens_in_the_browser_its_list_named`,
  `recording_status_in_an_empty_session_binds_no_browser` and `a_pinned_session_is_never_rebound`
  in `work/tests/availability.rs`; `a_fresh_service_waits_for_the_returning_browser_instead_of_recovering`
  now expects no binding after its list and waits on the open. Deleted
  `listing_tabs_waits_for_a_waking_relay_then_refuses_from_absence`, which tested the empty list's
  wait and refusal. No golden changed.
- C3a, journeys for the coordinator: `tests/cli-journey.mjs` (an empty list answers at once with
  exit 0; the no-browser refusal is now pinned through `browser_record` status) and
  `tests/process-journey.mjs` (an empty list asks the browser nothing). D2: LANGUAGE's "refuses
  without one" narrows to sessions that hold tabs, and ARCHITECTURE says the first tab binds the
  browser.
- C3a, the installed recovery journeys: an empty session's list no longer reaches the browser, so
  `tests/installed-windows-journey.ps1` and `tests/linux/installed-journey.mjs` poll the list until
  the service names exactly one connected browser, and after the crashed service returns, that
  same persistent browser id. That proves the new service registered the reconnected browser's
  connection. The Linux phase also opens, waits on and reads its fixture page over the same stream
  before it passes, so it crosses to the reconnected browser; the Windows journey's browser work is
  `tests/live-journey.mjs`, which it runs next. The coordinator runs both on a test machine.
- C4, the split: the first commit (e4f353ab) carries the aggregate rule, the disconnect and the
  registration, and also the lease's `confirm`, the per-binding browser and the token written
  from it, so the tree compiles without dead code. The second carries the executor's
  confirmation, the resume path and the digest in the token. Between the two, a pending binding
  read as no longer open; nothing was deployed there.
- C4, the registration: `RelayBrowserPort::register` tells the event sink `Disconnected` for the
  connection it replaces, before the new one is registered, because the replaced stream's own end
  is never reported. That is the one route by which "every registration of a browser id" reaches
  the aggregate; the event vocabulary in `browser/adapter/work.rs` is unchanged.
- C4, what a call selects: the wait for an absent browser runs for the pending binding the call
  would select. That is the tab it names, the tab of a target it names (a fill's field and submit
  targets and a drag's two ends included) or the tab of a view it names. A call that names none
  of them works in the workspace's only tab, so while the workspace holds no confirmed tab and all
  its pending bindings are in one browser, it waits for that browser and confirms them before it
  selects. That covers the service restart and the browser or native-host reconnect, so a
  `browser_read` or `browser_navigate` without `tab` keeps working in the session's tab and
  never opens a second one. A listing never waits (Decision 2), an open makes a fresh tab, and a
  flow step waits on its own frame, since each step runs the same confirmation. The wait ends the
  binding only when the browser did not come back (not connected, another profile, recovery left
  to the person or failed); a paused runtime, a silent connection, a spent deadline or a cancel
  answers the call with that refusal and keeps the binding pending.
- C4, a gap inside a call: a browser can restart between two crossings of one call, so every
  command addressed to one tab reads how the workspace holds that tab before it crosses
  (`WorkspaceStore::binding_state`). After the browser is chosen and before any presentation cue,
  a binding that went pending is confirmed with one `ListTabs`, and a tab the browser does not
  confirm answers "That tab is no longer open." with nothing sent (`BrowserError::StaleTab`). The
  port asks admission for a command that may change the page once it holds the connection it
  writes to, and admission refuses a tab that went pending in between as never sent. A read the
  port never asked about is confirmed again after its answer, and an answer from a tab the browser
  no longer confirms is dropped. The lease's `confirm` moved to the store, because a crossing
  holds no lease.
- C4, a listing that fails: a `ListTabs` changes nothing, so its failure after dispatch is read
  as the same failure before dispatch (`work/pending.rs`, `listing_failure`), never as an effect
  that may have happened.
- C4, "the result says so": a call whose first tab binds the workspace's browser while pending
  bindings of another browser wait reports their handles in a `forgotten_tabs` fact. No sentence
  changed, and no catalog or result golden changed.
- C4, a confirmed binding keeps its generation, handles and captures and takes the live title
  and readiness. Without the life (C21) only the URL confirms it, so the URL is the live one by
  construction.
- C4, the token: each tab carries `url_digest`, the SHA-256 of `TabState.url` bounded to
  `TAB_URL_CHARS` (8192, pinned to the protocol-4 vocabulary's `url_chars` by a test). It is
  republished after each call as before and now on the session watch's tick too, so a close or a
  commit outside a call reaches the token within a second. Bounding the token on write stays
  C11's.
- C4, proofs: in `work/tests/restarts.rs`
  `a_browser_restart_never_hands_an_agent_handle_the_persons_tab` (list, read, screenshot,
  navigate), `a_port_restart_keeps_a_tab_whose_page_is_unchanged`,
  `a_resumed_tab_is_confirmed_by_id_and_url_before_its_first_use`,
  `a_token_from_before_a_browser_restart_restores_nothing`,
  `a_call_on_a_tab_whose_browser_never_returns_forgets_it`,
  `a_call_that_names_no_pending_tab_leaves_it_pending_while_its_browser_is_away`,
  `opening_in_another_browser_forgets_the_resumed_tabs_and_says_so`,
  `a_gap_inside_a_call_never_sends_the_old_id_to_the_restarted_browser`,
  `a_gap_inside_a_call_keeps_a_tab_whose_page_is_unchanged`,
  `a_call_that_names_no_tab_waits_for_its_only_tabs_browser` (read, navigate) and
  `a_resumed_session_that_names_no_tab_waits_for_its_tabs_browser`; in `work/pending.rs`
  `a_fill_and_a_drag_select_their_tab_by_every_target_they_name` and
  `a_listing_and_an_open_select_no_tab`; in `workspace/mod.rs`
  `a_call_selects_the_pending_binding_it_names_or_the_only_one`,
  `a_crossing_reads_how_its_workspace_holds_the_tab_it_names`,
  `a_new_tab_with_a_recycled_id_forgets_the_pending_binding`,
  `a_commit_never_confirms_a_pending_binding`,
  `a_release_after_a_browser_restart_closes_no_pending_tab`,
  `a_disconnect_makes_every_binding_pending_and_inert` and the pool's
  `a_released_tab_is_reused_after_a_reconnect_only_where_its_browser_still_shows_it`; in
  `service/mod.rs` `a_release_after_a_browser_restart_never_closes_the_persons_tab` (no command
  reaches the browser, so preserve-tabs off closes nothing),
  `a_registration_that_replaces_a_connection_marks_its_bindings_pending`,
  `a_call_before_the_tokens_browser_returns_keeps_its_tabs_in_the_token`,
  `a_token_written_before_digests_resumes_with_stale_tabs`, and the rekeyed
  `a_resume_token_restores_the_session_and_stales_a_missing_tab` and
  `a_call_right_after_its_browser_returns_finds_its_resumed_tab`. Deleted
  `a_resumed_session_waits_for_its_browser_to_return`: the resume no longer waits.
- C4, journey for the coordinator: `tests/process-journey.mjs` gains a browser restart after the
  service restart. The browser edge exits, the world shows the person's page under the agent's
  old tab id, a new browser connects, and `browser_read` on the agent's handle reads "That tab is
  no longer open." with no call reaching the tab, and the list is empty. Run it from a fresh build
  named by `GHOSTLIGHT_BIN_DIR`. D2: ARCHITECTURE's confirmation rule and the inert pending
  binding, and README.md:106 with the same-id, same-URL residual.
- C5, the deadline: the ADR names one shared deadline and no length. `END_LIMIT` is 5 s from the
  end of the drain to the browser goodbye, so the 10 s drain and the end fit inside the 30 s an
  update start waits for the lock. A release command keeps its own 2 s (`RELEASE_CALL_LIMIT`) and
  is not sent once the end's deadline has passed; its tab then leaves the released pool as one
  whose close did not answer.
- C5, the order: the shells' goodbye goes out with the MCP goodbyes (step 2), before the release.
  The ADR names only the MCP and browser goodbyes.
- C5, who decides: `Farewell` holds the live MCP streams and, once a session opens, its workspace
  and queue. It takes over before the first goodbye, under the lock that enrolls streams, and from
  then on `LiveSession::close` releases nothing. A release a closing session began before the
  takeover runs to its end, and the end waits for it, under the same deadline, before the browser
  goodbye. After the takeover each stream's token is sealed: the session watch and a finishing call
  publish no token after the last one.
- C5, the cancel: the ADR says a leased workspace is cancelled first. The end stops the whole queue
  of every session whose workspace ends (`CancelCause::Quit`), not only its running call, so no
  queued call of an ended session starts after its release. Workspaces no call holds are released
  before the end waits for any held one.
- C5, the end of the session and the idle exit forget every workspace and the released pool
  without a browser command (`WorkspaceStore::forget_all`). The end of the session hands out no
  last token, because its edges exit on `session_end`.
- C5, a residual: a session accepted before a quit whose workspace opens after the takeover is not
  among the live sessions the end saw, so a replace releases it and its last token carries no tab.
  New openings are refused from the start of the drain, so only an opening already being read can
  meet this.
- C5, proofs: in `service/mod.rs`
  `a_replace_keeps_a_live_sessions_tabs_bound_grouped_and_resumable`,
  `a_quit_releases_every_session_and_hands_each_stream_a_token_without_tabs` (quit, uninstall and an
  unknown reason), `a_command_line_session_is_released_at_every_service_end` (the four quit reasons
  close its tab before the browser goodbye; the end of the session and the idle exit forget it with
  no command), `a_session_whose_client_left_during_the_drain_is_released_on_replace`,
  `a_leased_workspace_is_cancelled_before_its_release` (the cancel frame precedes the close, which
  precedes the goodbye) and `a_session_end_says_goodbye_inside_the_fallback_with_many_tabs` (300
  tabs, an extension that never answers). No golden changed.
- C5, journey for the coordinator: `tests/process-journey.mjs` gains a replace before its Quit. The
  session opens a tab, a `replace` quit closes nothing, the journey starts the replacing service as
  an update does, and a new browser edge's `browser_read` on the kept handle succeeds. The Quit
  then sends `close_tab` for that tab before the browser edge reports the service absent, and after
  the revival the handle no longer reads. Run it from a fresh build named by `GHOSTLIGHT_BIN_DIR`.
  `tests/installed-journey.mjs` check (26) said a quit keeps the earlier tab working, which
  Decision 3 retires: the first call after the quit now reads the earlier handle as no longer open,
  and a fresh tab opens and reads once the extension reconnects. The coordinator runs it on a test
  machine.
  D2: ARCHITECTURE's service end (the order, owned workspaces released at every end, the quit's
  token without tabs) and LANGUAGE's resume after a quit.
- C6, the hold: `TabState` keeps a `TabHold`: the landing's `Decision` (its reason and policy
  attribution), the id of the authority snapshot that made it, the landing's requirements and the
  refused URL. The URL stays in memory and is never audited. `WorkspaceError::Held` carries it, so
  `WorkspaceError` is no longer `Copy`. The executor's `hold` helper also returns the refused
  landing's result, so the four landing paths share one.
- C6, what the refusal records: under the authority that held the tab, the landing's decision, so
  the audit row has its reason, rule, denial id, tier and grant, and the result's facts carry the
  same `denial_id` and `policy_rule`. A decision's attribution is indices into the snapshot that
  made it, so under a republished authority the refusal decides the held URL again and records
  that decision. When the new authority admits the URL, the tab stays held (Decision 2.4) and the
  record keeps the hold's reason with no attribution. The audit's observed host is the refused
  host, and the retained summary names it, as an `AuthorityBlocked` summary does.
- C6, the audit cause: the model-side refusal is `Refusal::TabHeld { host }`, and it audits as
  `{"reason":"workspace_unusable","cause":"tab_held"}`, the bare word audit v2 already writes.
  `WorkspaceReason::TabHeld` stays a unit variant.
- C6, a notice: the decision is no longer `RuntimeHold`, so a call refused on a held tab outside a
  flow shows the ordinary guardrail notice, as any policy refusal does. The hold itself still
  shows its own notice once, when it is taken.
- C6, the words: the ADR names none. The model reads "That tab shows <host>, which this session may
  not use, so Ghostlight will not read or act on it there." with one step naming `browser_navigate`
  and `browser_tabs` close. The person's notice reads "Ghostlight stopped on this page", from the
  new `language/presentation.rs`, which C18 extends to the whole table.
- C6, proofs: `a_held_tab_refusal_names_the_refused_host_and_the_calls_that_clear_it`,
  `a_held_tab_is_audited_with_its_landing_denial_not_a_pause` and
  `a_flow_step_on_a_held_tab_is_blocked_by_policy` (the flow goes on to its second step, which a
  pause would stop) and
  `a_held_tab_refused_under_a_republished_policy_credits_the_policy_that_refused_it` (grants
  reordered, then the page admitted) in `work/tests/tabs.rs`;
  `a_held_tab_keeps_the_audit_cause_word_and_reads_back` in `language/audit.rs`. The catalog
  golden (`docs/1.0/catalog.json`) changed only in `user_authorized_credentials`'s "Pause or End
  session"; no result golden or audit golden changed. No journey is needed. D2: LANGUAGE's
  held-tab entry (the host, the two calls out, and a flow step blocked by policy, not paused).
- C6, a cross-lane accident: install's `e97d4618` committed `language/outcome.rs` whole, with
  core's unfinished held-tab hunks in it, so `e97d4618` alone does not build. Install's `77290406`
  took them back out, and C6's review fixup commits them with their dependents.
- C7, the conversion: `impl From<WorkspaceError> for WorkspaceReason` goes, and
  `language::outcome::workspace_refusal` gives each workspace error the refusal that is true of it
  and its `reason` fact word. The ADR calls the `BrowserPinned` arm unreachable; since C3a,
  `add_tab` and `restore_tab` raise `WorkspaceError::BrowserPinned` for a tab of another browser,
  so the variant stays, and its arm answers with `Refusal::BrowserPinned` ("This session is
  already working in a different browser.") instead of an ownership sentence. A held tab's host is
  read by `TabHold::host`.
- C7, the causes: `handle_on_other_tab` replaces `TargetTabMismatch` and `ViewTabMismatch`, and
  `WorkspaceError::HandleOnOtherTab` carries the tab the handle belongs to, which the sentence
  names and the `handle_tab` fact carries. `outside_image` replaces the two out-of-bounds errors,
  with the view's size in the sentence and in an `image` fact; a view whose output scale is not
  positive is now a stale view, because nothing about the point is wrong. `fields_on_several_tabs`
  is raised by the fill itself: a field target or submit target is resolved in the tab the call
  named, or in its own tab, so a fill that names no tab is told the fill works in one tab, and a
  fill that names one is told which tab the handle belongs to. In the audit each is a bare cause
  word, like `tab_held`: the carried tab and size are skipped when serialized, `retained()` drops
  the tab from the durable sentence, and the size stays there as a measurement.
- C7, the words: `TabUnavailable` reads "That tab handle names no open tab in this session.",
  true for a closed, forgotten or never-issued handle. The ownership refusal reads "That handle
  belongs to another Ghostlight session (Claude Code).", naming the owner's agent from its opener's
  `ConnectionEvidence::details()`, as the window does, and no one for the unknown agent. The
  `owner_workspace` fact carries the same name; C13 reads the name once at workspace open.
- C7, the flow: a step whose every tab-scoped part is a target or view handle (`target`, `root`,
  `view`, `submit_target`, `source_target`, `destination_target`, a fill field's `target`) gets no
  flow tab. A selector names no tab, so a step with one keeps the flow tab, a fill whose
  `submit_target` or other fields are handles included; a handle among them on another tab is then
  refused as `handle_on_other_tab`. The catalog's flow sentence ("a step's own tab wins") is unchanged, because the ADR counts
  one changed catalog phrase.
- C7, proofs: `a_target_from_another_of_this_sessions_tabs_names_that_tab`,
  `a_fill_across_two_tabs_says_a_fill_works_in_one_tab` and
  `an_ownership_refusal_names_the_owner_as_the_window_does` in `work/tests/tabs.rs`;
  `a_point_outside_the_image_gives_the_image_size` in `work/tests/execution.rs`;
  `a_flow_tab_is_not_injected_into_a_step_whose_handle_names_its_tab` in `language/mod.rs`;
  `workspace_errors_map_to_the_refusal_that_says_what_is_true` and
  `a_handle_on_another_tab_a_split_fill_and_a_point_off_the_image_each_say_so` in
  `language/outcome.rs`. The catalog, result and audit goldens were regenerated and did not
  change: none carries these refusals.
- C7, journey for the coordinator: `tests/process-journey.mjs` now expects the new
  `TabUnavailable` sentence after the browser restart. Run it from a fresh build named by
  `GHOSTLIGHT_BIN_DIR`. D2: LANGUAGE's refusal table (`tab_unavailable`'s sentence, the ownership
  sentence, the three new causes); the SIEM guide's three cause words; the ADR's Consequences and
  the owner's try-list say "no longer open" informally, and the sentence now says the handle names
  no open tab.
- C13, the claim: `provenance::bounded_claim` is the one bound (control and direction text
  stripped, 100 characters, trimmed, empty is none). `McpSession` keeps `claim: Option<String>`
  through it when it reads `initialize`, discovery and a resume token, and `ConnectionEvidence::new`
  takes `Option<&str>` through the same function. The token keeps the key `client_label` and omits
  it for no claim, so a token an earlier build wrote still resumes its client's name. A token
  written by an earlier build for a client that sent no name carries "MCP client", and resumes as an
  unknown agent of that name, under the group title it already had.
- C13, a live connection with no claim: `language::provenance::details` takes `Claimed::Live` or
  `Claimed::Unkept` instead of an optional name, because with the placeholder gone an absent name
  no longer told a live connection from a record. A live claimless connection stays the unknown
  agent, never the earlier session's.
- C13, read once: `WorkspaceState.named` holds an `Opener { agent, reuse_key }`, built when the
  workspace opens. The group title, the ownership refusal and reuse read it. The session card still
  reads `opener.details()`, the same function over the same evidence and the static roster, so it
  names the same agent and keeps a current icon. `WorkspaceStore::opener_claim` became `opener`.
- C13, the pool: a workspace with no reuse key leaves no tab in the released pool at all, since no
  one may claim it. Its tabs are still released and closed as preserve-tabs says.
- C13, the title: `language::provenance::group_title` writes "Ghostlight - " and the agent's name,
  bounded to 80 characters as before. Agent names, not claims, now title groups, so a client that
  claims "claude-code" gets "Ghostlight - Claude Code" and a command-line session gets
  "Ghostlight - Command line", where it got "Ghostlight - ghostlight call". The popup's title parse
  reads the right name with no extension change.
- C13 absorbs C22: removing `claimed_name`'s decode, which C13 deletes, made the uses C22 names
  (`agent/mod.rs`, `workbench/tests.rs`) assert something false, so they changed in this commit to
  read no claim (`None`, or a name that bounds to nothing), and `DEFAULT_CLIENT_LABEL` went with
  them. One `ConnectionDetails` literal in `workbench/parts/tests.rs` gained `source_agent`. These
  are test-only cross-lane edits; no surface or install source changed.
- C13, proofs: `a_group_is_titled_for_the_agent_the_window_names`,
  `an_opener_the_catalog_cannot_name_is_titled_unknown_agent`,
  `a_client_that_names_nothing_reuses_no_kept_tab`,
  `a_claimless_client_started_by_claude_code_reuses_no_claude_code_tab`,
  `a_command_line_session_reuses_a_tab_another_command_line_session_kept` and
  `a_tab_a_command_line_session_kept_is_not_reused_by_claude_code` in `work/tests/tabs.rs`;
  `an_empty_name_and_no_name_stay_the_same_unknown_agent_after_a_resume` in
  `intake/mcp/tests.rs`; `connection_details_carry_the_sources_catalog_agent_unserialized` and
  `a_group_title_is_the_agents_name_after_ghostlight` in `language/provenance.rs`;
  `a_reuse_key_is_the_agent_the_claim_names_and_never_the_source` in `provenance.rs`;
  `a_workspace_whose_claim_names_no_agent_leaves_no_tab_for_reuse` in `workspace/mod.rs`. No
  catalog, result or audit golden changed. No journey checks a group title, so none changed and
  none is needed. D2: ARCHITECTURE's opener-claim lines, INTENT's and the installation guide's
  `Ghostlight - <client label>`, LANGUAGE's reuse rule and ACCEPTANCE's Extension product gates
  item 8 now read the opener's agent name.
- C9, one rule for both cases: the wait computes its end first and refuses at once, as
  `DeadlineBeforeDispatch`, when that end is past the deadline. A sleep that starts therefore always
  ends before the deadline, so the loop checks only cancellation, and the after-dispatch deadline
  goes with no before-dispatch branch in its place. The refusal uses the existing deadline sentence
  and its `timeout_ms` advice; the duration wait's own sentence stays C17's.
- C9, tidy-up in the same branch: a duration wait is always satisfied, so its constant
  `final_satisfied` and the status chosen from it went. The 20 ms poll is the named
  `DURATION_POLL`.
- C9, test harness: `work/tests/protocol4.rs` gained `opened_replying`, which takes a script that
  may hold a reply back or never give one; `opened_audited` now wraps its fault into it.
- C9, proofs: `a_duration_wait_past_its_deadline_is_repeat_safe_and_says_nothing_was_sent` and
  `a_duration_longer_than_the_time_left_is_refused_without_sleeping` in `work/tests/execution.rs`;
  `a_download_save_that_outlives_its_deadline_is_unknown_and_not_repeat_safe` (A2's work-level
  proof) and `no_embedded_host_reaches_the_permission_trace_from_a_targeted_read` (A1's) in
  `work/tests/protocol4.rs`. No catalog, result or audit golden changed. No journey is needed.
- C10a, a tab already gone: the ADR does not name it. Its close has no page to check Action on, so
  the person's runtime controls and the tab-close setting decide it, as for a held tab; the setting
  still refuses it with "this session may not close tabs", which stays true for a gone tab.
- C10a, runtime controls: a held tab's close asks the person's runtime controls before the setting,
  so a paused session gets the pause's answer. Compensation asks the setting alone; a pause still
  stops its close at the browser seam, which keeps the tab as retained, as before.
- C10a, a retained landing (review fixup): when compensation keeps the refused page open (the
  setting refuses, the local interlock or a pause stops the close, or the send fails before
  dispatch), the tab is held through the one hold helper, `enter_hold`, with the landing's denial,
  Read and the refused address, and `HoldEntered` is emitted. Its later close is decided by the
  setting alone, and its refusals and listing name the refused host. The retained result now
  carries `tab` and `held` beside `compensated` and `retained`, and its sentence names the host.
- C10a, the refused close: Action refused on the tab's host keeps the existing capability sentence
  and records the page's host as observed. The setting's evidence carries that host too, and none
  for a held tab or compensation.
- C10a, the listing: a row for a tab on a refused host keeps `tab`, `active` and `readiness`, names
  `host` (null when the address has none, such as an ungoverned blank tab) and gives the refusal's
  `reason` code, with no `title` or `url`. The check is the snapshot's `authorize_landing(Read,
  url)` on the live address and adds no evidence to the frame. The listing's own admission stays
  the host-free Read check, and `TabsListed` still counts every bound tab it names.
- C10a, the false comment: `authorize`'s doc no longer says `list_tabs` is the only host-free
  caller; it says what `url: None` decides.
- C10a, proofs: `closing_a_tab_needs_action_on_its_host`,
  `a_denied_landing_is_closed_under_a_read_only_policy_when_the_setting_allows`,
  `a_held_tab_is_closed_under_the_setting_alone`,
  `a_retained_denied_landing_is_closed_under_the_setting_alone` and
  `listing_withholds_title_and_url_of_a_tab_on_a_refused_host_and_names_the_host` in
  `work/tests/tabs.rs`. No catalog, result or audit golden changed. No journey is needed.
- C10b, a source may be embedded: the recording's source list holds every document the capture
  saw, embedded ones included, and an embedded host never enters the durable record (Decision
  6.1). So each source is checked once on the frame's trace through `authorize_document`, the
  frame's landing check with the host left off, rather than through `authorize_landing`, which
  would write every source's host into the audit. The refused hosts still reach the person's
  coverage notice. The dialog check in `work/documents.rs` uses the same helper in place of its
  copy.
- C10b, the admission helpers: `with_authorized_tab`, `with_authorized_optional_target` and
  `targets_operation` lose their capability parameter and admit `context.requirements`, so no
  caller can name a capability of its own. A model close checks `context.requirements` on its
  host. Only an upload by selector changes what it checks: Read and Write, as classified, in place
  of Write alone. A fill's check before it resolves its selectors asks the classified set too.
- C10b, staged checks stay: an action's landing on the pages it committed asks for Action alone,
  a recording placed on a page asks for Write on its destination, and a held or gone tab's close
  asks the runtime controls alone. These are later stages under their own requirement, not
  admissions.
- C10b, proofs: `every_single_stage_tool_is_admitted_against_its_classified_requirements`, which
  runs every catalog variant bare and in a flow and lists the staged variants it lets check a
  later stage, and `a_refused_recording_save_records_its_deciding_check`, in
  `work/tests/catalog_authority.rs`. No catalog, result or audit golden changed. No journey is
  needed.
- C11, the cause: `CancelCause::Quit` carries a closed `QuitCause` in `language/outcome.rs`. It
  names the four quit reasons and the two other ends that begin the drain, the end of the sign-in
  session and the idle exit, because the drain now keeps the cause its quit began with and a call
  refused by it reads that cause. A call cancelled before it started gets "Try the call again in a
  moment." after a quit, a replace, an idle exit or an unknown reason (the edge reconnects after
  each), and no step after an uninstall or a sign-out (the edge exits). Only
  `CancelledBeforeStart` gains the step; a call cancelled before or after dispatch keeps its
  steps. The summary and the audit word are unchanged, and `EdgeFailure::Expired` reads the same
  step from its one constant.
- C11, the CI race: a session that opened before the quit's end had its queue stopped by the end,
  and a call that arrived after that met `Refused::Closed`, which ended the stream with no answer.
  The queue now keeps the cause it stopped for, and `admit` refuses with
  `Refused::Cancelled(cause)` whether the queue stopped or the drain refuses. `LiveSession::handle`
  answers every call it does not queue on the thread that reads the stream, through
  `execute_prepared` with `Admission::Cancelled(cause)`, before the loop can reach the stream's
  close, and no longer ends the session. A call cancelled at admission takes no lease and skips the
  channel preflight, so it says it was cancelled.
- C11, the token with `initialize`: it is written ahead of the `initialize` answer, before the
  session's source is read, and even after a service end sealed the stream: a fresh session holds
  no tab, so it is the token the end would hand it. Before, a stream sealed before its
  `initialize` got no token at all, and a held source read delayed it.
- C11, the bound: `ResumeState::encode` never writes past `MAX_RESUME_TOKEN_BYTES` (64 KiB, what
  `decode` reads). A state too large keeps its first tabs, as many as fit; a tab left out reads as
  no longer open after a restart.
- C11, an unreadable token resumes `ResumeState::unreadable()`: the oldest revision, no claim, no
  tab. Its diagnostics line is `harness_attached` at warn with the decode error, an existing event
  (the vocabulary is install's bridge). The test hub's sink is off, so no test reads the line.
- C11, proofs: in `service/mod.rs` `a_call_during_a_quit_gets_a_tool_result_and_an_audit_line`
  (quit, replace and an unknown reason, each once while the drain waits on work in flight and once
  after the end stopped the queue, with no sleep: each step waits on an answer or the quit signal),
  `a_call_during_an_uninstall_gets_no_retry_step`,
  `an_unreadable_token_resumes_an_initialized_session_without_tabs` and
  `an_answered_initialize_always_has_a_token` (a held source read, and a stream sealed by a
  replace before its `initialize`); `a_quitting_service_takes_no_new_connection` keeps the old
  test's readiness check. In `intake/mcp/tests.rs`
  `an_unknown_revision_resumes_at_the_oldest_supported_revision`,
  `an_unreadable_token_resumes_an_initialized_session_at_the_oldest_revision` and
  `a_token_never_outgrows_the_opening_cap`; in `service/admission.rs`
  `a_stopped_queue_refuses_later_requests_for_the_cause_it_first_stopped_for`; in
  `language/outcome.rs` the next steps for every `QuitCause`. Deleted
  `a_quit_answers_new_invocations_with_shutting_down`,
  `a_service_error_for_a_call_is_an_internal_error_naming_its_code` and
  `the_quit_refusal_is_plain_ascii_and_says_nothing_was_sent`; the bound of a protocol refusal's
  message is now proved on the parse error's detail. No catalog, result or audit golden changed.
  No journey is needed: the service tests run over real relay streams. D2: LANGUAGE says a call
  during a quit is a cancelled tool result with its retry step, and ARCHITECTURE says the token is
  published with the `initialize` answer, bounded, and that an unreadable token resumes at the
  oldest revision with no tab.
- C12a, the class: `language::history::OutcomeClass` has the ADR's seven words, serialized as
  `succeeded`, `refused`, `kept`, `needs_you`, `failed`, `unknown` and `cancelled`.
  `OutcomeClass::of(status, refusal)` classes one settled call: a credential refusal is
  `needs_you`, the preserve-tabs refusal (`LocalInterlock`) is `kept`, and otherwise the status
  decides, so a blocked call is `refused`. The window is not changed here; S2 sets
  `HistoryItem`'s class from `OutcomeClass::recorded`.
- C12a, the record: `OutcomeClass::recorded` takes the `AuditRecord`, because a flow keeps no
  refusal of its own. A blocked flow whose decision refused it is `refused`. A blocked flow that
  authority admitted is classed by its deciding step's cause: a refused check, a pause or an end
  is `refused`, `attention_required` is `kept`, and any other cause is `needs_you`, because only a
  credential step blocks without a refusing decision. The stored status is read by the new
  `Status::from_stored_name`. A word no `Status` names, the step that never started, is `failed`;
  C19a spells that word once.
- C12a, the flow causes: `OutcomeClass::step_cause` gives each class its cause, refined by the
  step's refusal for the person's controls, the history requirement, the deadline, a cancellation,
  a lost connection and an unconfirmed check. Every cause a flow kept before is kept: a kept tab
  stops the flow as `attention_required`, and a credential refusal continues as `failed`, as
  `continue_honors_human_attention_and_invocation_limits` and
  `credential_guidance_obeys_flow_continue_without_pausing_later_work` pin. The ADR does not decide
  a change there. The preserve-tabs refusal tells the agent to continue while a flow stops on it,
  and that was already so before this slice.
- C12a, proofs: `a_credential_refusal_is_classed_needs_you_and_a_kept_tab_kept` (single calls,
  their records and flow records), `every_status_has_its_class_and_a_step_that_never_started_failed`
  and `the_class_crosses_to_the_window_as_one_closed_word` in `language/history.rs`. Composition's
  own cause mapping and `unconfirmed_cause` moved there. No catalog, result or audit golden
  changed. No journey is needed.
- C19a, one writer of the line: `AuditRecord::not_started` builds the whole line of a step that
  never started, `with_policy` included, so it carries `policy_seq` under managed governance as
  every other line does. The executor's `record_preparation` calls it and no longer spells a status,
  an effect or a decision of its own. Its decision is still `invalid_request` with no attribution,
  so only `policy_seq` is added to the line.
- C19a, the words: `Effect::stored_name` sits beside `Status::stored_name`, and both read one
  serialized-name helper in `work/result.rs`. `UnexecutedStatus` moved from `work/composition.rs`
  to `work/result.rs` and is the one spelling of `not_started`, for the flow row and the audit
  line. The audit tests' own `wire` helper went for the two stored names. Not changed, because
  they are other lanes' readers of the stored word: the surface test literal in
  `workbench/history.rs` and the window's label in `ui/lib/view.js`.
- C19a, cross-lane: `GovernanceFacade::with_managed_paths` (test-only) became `pub(crate)` so a work
  and an audit test can run under a signed organization policy. The policy lane is closed.
- C19a, proofs: `a_not_started_step_carries_policy_seq_under_managed_policy` in
  `work/tests/execution.rs` (the step's line and the flow's both carry the sequence) and
  `the_not_started_line_matches_its_golden` in `audit/record_tests.rs`. The audit golden
  `tests/golden/audit/not-started.json` is new, written with `GHOSTLIGHT_AUDIT_GOLDEN_UPDATE=1`,
  and also joins the unknown-fields read test. The other audit goldens and the catalog and result
  goldens did not change. No journey is needed. D2: the SIEM guide may name the not-started line
  and say it carries `policy_seq` under managed governance.
- C21, where the life is kept: on each registered connection, from its hello, and
  `BrowserPort::life_of` reads it (the default knows none, so a fake or a port without lives
  confirms by URL). A connection that ends or is replaced reports its life in
  `BrowserEvent::Disconnected { life }`, so the bindings that go pending take the life of the run
  they were confirmed in from the event, not from a registry a new hello may already have
  changed. The ADR says the registry keeps the last life for each browser id; nothing reads the
  life of a browser that is not connected, so no map outlives its connection. A hello whose life
  is not an opaque `life_` id is taken as naming none.
- C21, what a binding records: a pending binding remembers its URL digest and its life
  (`Remembered`). A confirmed binding is in the life its browser's connection reports now, so it
  records none until it goes pending. Recording it at bind time would need the life at every
  `add_tab` and `restore_tab` call, surface's tests included, for a value the port already holds.
  The resume token reads a confirmed binding's life from the port, before it reads the bindings: a
  gap marks a browser's bindings pending before a new connection replaces the old one, so a
  binding still confirmed after that read was confirmed in the life it returned.
- C21, the token: `ResumeState.lives` maps each browser of the kept tabs to its life, as the ADR
  says ("a life per browser"). A browser whose tabs name two lives, or a confirmed tab with no
  connection, carries none, so its tabs are confirmed by URL. A token cut to fit keeps only the
  lives of its kept tabs' browsers. An earlier token has no `lives` and resumes by URL.
- C21, the confirmation: the life is read once the listing answered, so it names the run the
  listing came from or a later one, never an earlier one. In the binding's own life the tab with
  its id confirms it and it takes the live URL, title and readiness. Its generation moves on when
  the live URL is not the remembered digest, so a target or capture of the earlier page reads as
  navigated; a changed title alone moves nothing. A hold stays as it was, because only a governed
  landing clears it.
- C21, the released pool keeps the URL rule. Its entries record no life, and a kept tab whose page
  routed itself is then not reused and the open makes a fresh tab, which costs a tab and never
  hands over the person's. The ADR's C21 Changes name only pending bindings.
- C21, cross-lane: the adapter lane's `protocol4/wire.rs` (`Hello.life`), `protocol4/connection.rs`
  and its tests, `browser/adapter/work.rs` (the event's field) and the fixtures
  `tests/golden/protocol4/frames/hello.json` and `tolerance/hello.extra_fields.json`, which gained
  `life`. The adapter lane is closed. `assets.sha256` and the package record did not change, and
  the extension's tests pass unchanged, as B1's notes say.
- C21, proofs: `a_worker_restart_in_the_same_life_keeps_a_tab_whose_page_routed_itself`,
  `a_new_life_falls_back_to_the_url_check` and
  `a_token_or_hello_without_a_life_falls_back_to_the_url_check` in `work/tests/restarts.rs`;
  `a_service_replace_in_the_same_life_keeps_a_routed_tab_and_takes_its_live_url` and
  `a_service_replace_into_a_new_life_forgets_a_routed_tab` in `service/mod.rs`, through a relay
  port and a protocol-4 fake extension;
  `in_its_own_life_a_pending_binding_is_confirmed_by_id_and_a_routed_page_moves_on`,
  `in_its_own_life_an_unchanged_page_keeps_its_generation_and_targets`,
  `another_life_or_no_life_confirms_only_by_url` and
  `a_resumed_binding_remembers_the_life_its_token_names` in `workspace/mod.rs`;
  `the_port_reads_the_life_each_hello_names_and_each_departure_carries_it` and
  `a_hello_without_a_readable_life_is_accepted_and_names_none` in `protocol4/connection/tests.rs`;
  `a_hello_without_a_life_decodes_and_serializes_without_one` in `wire.rs`; the token tests in
  `intake/mcp/tests.rs` carry lives. No catalog, result or audit golden changed.
- C21, journey for the coordinator: `tests/process-journey.mjs` gives its synthetic browsers a life
  (`SyntheticBrowser.life` in `tests/lib/peers.mjs`): the first run's browsers share one, the
  browser after the restart gets another, and during the replace the kept tab's page routes itself.
  The replacing service resumes it in the same life and reads it at its routed address. Run it from
  a fresh build named by `GHOSTLIGHT_BIN_DIR`; it was not run here. On a test machine with B1's
  extension: a worker restart (`chrome://serviceworker-internals` stop) and a service replace keep
  a single-page app's tab bound, and a browser restart leaves its handle naming no open tab.
  `tests/golden/agents/native/linux.txt` is unchanged. D2: the adapter-protocol-4 design and
  ARCHITECTURE's hello (`life`), and the confirmation rule with the life.
