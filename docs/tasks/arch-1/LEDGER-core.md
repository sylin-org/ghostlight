# arch-1: core ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

C1, C2, C3a and C4 have landed. C5 is next. ux-2 has closed, so every slice whose "After" is met
can start.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| C1 | fix(audit): the durable record keeps only what may be retained | candidate | ux-2 closes | see Notes | landed (fe18e597) |
| C2 | fix(workspace): a tab binding has one lifecycle | candidate | ux-2 closes | see Notes | landed (7bd0bf20) |
| C3a | fix(work): a session's browser is bound by its first tab | candidate | C2 | see Notes | landed (ec15bacb, journey fixup) |
| C4 | fix(workspace): a remembered tab is pending until its browser confirms it | candidate | C3a | two commits; see Notes | landed (e4f353ab, this commit) |
| C5 | fix(service): a service end decides its sessions' tabs once | candidate | C4 | - | todo |
| C6 | fix(language): a held tab is described truthfully | candidate | C2 | - | todo |
| C7 | fix(language): a workspace refusal says what is true | candidate | C6 | - | todo |
| C13 | fix(provenance): the opener is named once | candidate | C7 | - | todo |
| C9 | fix(work): a duration wait says what its deadline did | candidate | A1, A2 | - | todo |
| C10a | fix(work): closing and listing a tab look at its host | candidate | C2 | - | todo |
| C10b | fix(work): every authority check of a frame is on the frame's path | candidate | ux-2 closes | - | todo |
| C11 | fix(service): a call during a quit and a token that cannot be read both get an honest answer | candidate | ux-2 closes | - | todo |
| C12a | feat(language): history rows carry one closed outcome class | candidate | ux-2 closes | - | todo |
| C19a | fix(audit): a step that never started carries the policy sequence | candidate | ux-2 closes | - | todo |
| C21 | feat(adapter): the service reads the browser's life | candidate | C4, B1 | lands with B1; cross-lane `protocol4/wire.rs` | todo |
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
| C22 | refactor(intake): the "MCP client" placeholder goes | none | C13 | cross-lane `agent/mod.rs`, `workbench/tests.rs` | todo |

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
- C4, what a call selects: the wait for an absent browser runs when the call names a pending tab
  handle, or a target handle on a pending tab. A call that names neither selects only a sole
  confirmed tab, because a pending binding is inert, so it never waits. A flow step waits on its
  own frame, since each step runs the same confirmation. The wait ends the binding only when the
  browser did not come back (not connected, another profile, recovery left to the person or
  failed); a paused runtime, a silent connection, a spent deadline or a cancel answers the call
  with that refusal and keeps the binding pending.
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
  `a_call_that_names_no_pending_tab_leaves_it_pending_while_its_browser_is_away` and
  `opening_in_another_browser_forgets_the_resumed_tabs_and_says_so`; in `workspace/mod.rs`
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
