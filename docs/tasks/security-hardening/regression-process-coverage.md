# Hardening regression coverage: process, authority, and human history

This is the behavior-to-test inventory for H1, H2, H4, H5, H7, H8, and the accepted C1
reporting foundation. The suite's execution report owns the build and platform results.
H3, H6, editor mechanics, and the complete tool/capability matrix have additional browser
and executor coverage in the overall regression record.

Tests deliberately distinguish an allowed operation, a denied operation, actual dispatch,
retained effects, and the receipt. A successful tool envelope alone is insufficient evidence.

## H1: readable, content-minimized audit

| Promise | Executable check |
| --- | --- |
| A failed composition can return permitted earlier page content to its caller without retaining that content or browser exception text. | `tests/process-journey.mjs`, H1 failed Read/script flow and actual JSONL assertions; `work::tests::failed_flow_audit_excludes_prior_read_and_error_payloads`. |
| Primitive failures, script results, invalid input context, and unknown tool names cannot turn durable history into a content archive. | `work::tests::audit_excludes_primitive_exception_text_and_unknown_tool_names`; `work::tests::audit_excludes_script_results_and_invalid_input_context`; process JSONL and diagnostics `PRIVATE_` sentinels. |
| Readable measurements and governed target names survive; policy can remove names. | `language::audit::tests::retained_language_preserves_useful_context_and_governed_names`; `work::tests::governance_can_remove_target_names_without_losing_the_safe_role`; process host/count/readiness assertions. |
| Every advertised operation has a stable safe audit name. | `language::audit::tests::every_catalog_tool_has_a_stable_audit_name`. |

The named `work::tests` cases live in
[`work/mod.rs`](../../../crates/orchestrator/src/work/mod.rs). Language projection cases live in
[`language/audit.rs`](../../../crates/orchestrator/src/language/audit.rs).

## H2: composition stopping, progress, and recovery

| Promise | Executable check |
| --- | --- |
| Default Stop ends at a denied child without undoing earlier effects. | `flow_stops_after_a_refused_child_without_reverting_prior_effects`; process `blockedFlow`, including its explicit never-run third step. |
| Stop and Continue differ after runtime argument and reference failures; unable-to-start is distinct from never-run. | `flow_error_policy_controls_runtime_argument_failures`; process `invalidFlow` and its preparation-failure JSONL receipt. |
| Continue can complete later independent work but cannot erase a previous denial/failure or inflate completed counts. | `continue_reports_policy_denial_after_later_independent_success`; process `mixed`, with later success, failed parent status, exact counts, and MCP `isError`. |
| Known applied and partial effects survive later failure or uncertainty in both composition forms. | `work::composition::tests::mixed_progress_preserves_known_effects_and_never_promotes_failure`; `actual_unknown_causes_and_human_directives_survive_composition`; `direct_and_sequence_actions_use_the_same_physical_executor_path`. |
| Result-size omission preserves step metadata and reference resolution. | `flow_result_budget_preserves_step_metadata_and_reference_values`. |
| Only a fully successful, effect-free composition can be repeat-safe. Human attention, cancellation, and original deadlines override Continue. | `read_only_compositions_are_repeat_safe_only_when_fully_successful`; `continue_honors_human_attention_and_invocation_limits`; process cancellation after dispatch asserts unknown effect and observation-only recovery. |
| Returned progress and durable safe progress describe the same work. | Process JSONL parent `composition == facts.progress`, successful count, child order, and no private payload assertions. |

The shared progress tests live in
[`work/composition.rs`](../../../crates/orchestrator/src/work/composition.rs).

## H4: grouped history and permission evidence

| Promise | Executable check |
| --- | --- |
| A child receipt is saved before a later child and the parent finish. | Process `incrementalRequest`: read actual JSONL while the second child is still waiting; first child exists and parent does not. |
| Direct, child, and parent receipts retain their actual authority; failed preparation has its own receipt and never-run work has none. | `direct_and_composed_reads_keep_equivalent_safe_receipts`; `child_receipts_share_authority_and_count_denials_once`; process group count/order/shared-authority/permission assertions. |
| Positive, observed, request-restricted, and refused permission explanations reflect the evaluation that happened. | `governance::evidence::tests::positive_evidence_preserves_ordered_grants_and_every_layer`; `all_open_restrictions_observe_and_protected_denials_keep_their_actual_meaning`; `trace_bounds_and_deduplication_keep_omission_explicit`. |
| Missing receipts and missing parent completion never become fabricated execution or running work after reload. | `workbench::history::tests::groups_preserve_incremental_evidence_and_never_guess_missing_execution`; `child_receipt_updates_do_not_settle_parent_or_notify_and_reload_equally`; workbench surface restored-history checks. |
| History retention evicts complete groups and tolerates duplicate receipts without duplicate work. | `workbench::history::tests::history_limit_evicts_whole_groups_and_duplicate_receipts_do_not_expand_them`. |
| Details start collapsed, open at the relevant problem, and retain focus, expansion, and scroll during updates. | `tests/workbench-history-browser.mjs`: real bundled UI, overflowing 20-step group, permission details, incremental update, and 720-pixel layout. |

History reconstruction tests live in
[`workbench/history.rs`](../../../crates/orchestrator/src/workbench/history.rs).

## H5 and the editor incident: reliable controls and usable diagnostics

| Promise | Executable check |
| --- | --- |
| Repeated enforced denials affect only the originating session. Global Resume does not clear its incident. | `automatic_attention_is_local_reviewed_and_never_replays_work`; process troubled MCP session, unaffected second session, and retained attention after global Resume. |
| Human recovery targets the exact incident; stale recovery cannot clear a newer incident or global Pause/Stop. No browser work replays. | `automatic_attention_is_local_reviewed_and_never_replays_work`; actual bundled UI cleared-history Review and scoped Resume assertions. |
| Observe mode cannot relax a caller's explicitly narrower request limits. | `observe_admission_still_enforces_direct_and_composed_request_limits`. |
| A control change after preparation prevents the effect; a change after dispatch cannot claim rollback. | `human_controls_after_preparation_prevent_the_effect_in_direct_and_composed_work`; `pause_before_postcondition_preserves_the_confirmed_action_and_audit_effect`; `control_after_dispatch_preserves_applied_and_uncertain_receipts`; process Pause between focused-control description and typing proves no typing dispatch. |
| Diagnostic explanation stays available while session attention, Pause, Stop, or strict audit failure prevents browser work, and never releases any hold. | `policy_explain_remains_available_without_releasing_attention_or_human_controls`; process attention explanation with a narrow caller allowlist followed by a still-blocked browser call; H8 explanation after Stop. |
| Diagnostic availability does not override cancellation or extend an original deadline. | `policy_explain_still_honors_cancellation_and_original_deadlines`. |
| Draining work after a deliberate Pause/Stop produces quiet history without a popup per request. | `human_pause_and_stop_drain_work_without_repeated_guardrail_popups`; H8 real-service queue drain checks both controls. |

These executor tests live in
[`work/tests/control.rs`](../../../crates/orchestrator/src/work/tests/control.rs).

## H7: storage truth and bounded recovery

| Promise | Executable check |
| --- | --- |
| Browser success and uncertainty retain their effects when storage fails; Keep working allows later useful work. | `audit_failure_preserves_success_unknown_effects_and_default_continuation`; process replaces the actual audit file with a directory, then asserts succeeded/applied/unsafe-to-repeat/unconfirmed navigation and a successful subsequent read. |
| Require audit stops subsequent browser dispatch, including in observe mode; diagnostic and human controls remain available. | Process actual storage outage, mandatory policy reload, no-dispatch audit refusal, policy explanation, Pause and Resume; `audit_health_is_rechecked_after_an_earlier_admission`. |
| A storage failure discovered after a child effect ends a strict composition even under Continue. | Process `strictFlow`: first navigation acknowledged, second blocked, third never-run; exact one physical open; partial effect; two unconfirmed children and unconfirmed parent. `strict_audit_stops_later_flow_work_under_continue_and_keeps_prior_effects` checks the owning seam. |
| Repair enables new work but never repeats browser actions, backfills missing receipts, or upgrades unconfirmed child history. | Process two real outage/repair cycles and JSONL gap assertions; `audit::tests::failed_receipts_stay_unconfirmed_after_bounded_recovery_without_backfill`; `workbench::history::tests::saved_parent_never_upgrades_an_unconfirmed_child_receipt`; bundled UI child storage remains unconfirmed after parent save and recovery. |
| Cold storage failure does not prevent service startup or diagnosis. | Process final cold restart with an unreadable audit destination, successful `policy_explain`, and pre-browser strict refusal. |
| Corrupt/oversized lines do not hide valid neighboring receipts; writing again does not imply historical gaps were recovered. | `audit::tests::actual_file_repair_keeps_good_history_and_reports_torn_and_missing_entries`; `oversized_history_does_not_hide_the_following_receipt`; bundled UI recovery-gap and unreadable-entry notices. |
| Concurrent failures share bounded, content-free recovery state. | `audit::tests::cold_failure_and_concurrent_receipts_share_one_recovery_state`; `recovery_markers_and_health_round_trip_without_payloads`; process gap records exclude host content. |

Storage tests live in [`audit.rs`](../../../crates/orchestrator/src/audit.rs) and
[`work/tests/audit_health.rs`](../../../crates/orchestrator/src/work/tests/audit_health.rs).

## H8: continuity under bounded pressure

[`tests/local-resilience-journey.mjs`](../../../tests/local-resilience-journey.mjs) owns these
real-service scenarios with independent synthetic clients and browser receipts:

- An ordinary 12-request burst completes without intervention.
- Full per-connection admission refuses only overflow, while diagnostics and another session work.
- Queued cancellation and expiry finish without waiting for a stalled browser action; their effects
  remain none and the original deadline is not extended.
- Duplicate request rejection preserves cancellation of the original in-flight request. A missing
  browser receipt stays uncertain and is never replayed.
- Abandoned, malformed, oversized, and incomplete native peers expire; healthy idle peers survive.
- A client that stops reading is retired; other clients and a fresh connection remain usable.
- Pause and Stop drain queued requests before dispatch. Stop preserves its pinned directive and no
  retry action. Ordinary Resume cannot undo Stop; explicit Start Session permits only new work.
- A replacement runtime file is private: exactly the current owner and SYSTEM have explicit Windows
  FullControl; Unix permission bits are exactly 0600. Existing broad inherited permissions cannot
  survive replacement. This is a file-permission check, not cross-user impersonation proof.

The deterministic admission tests `duplicates_keep_the_original_token_and_queued_cancellation_bypasses_work`,
`bursts_are_fifo_and_full_work_preserves_controls_and_another_session`, and
`memory_and_connection_reservations_release_on_every_exit` live in
[`service/admission.rs`](../../../crates/orchestrator/src/service/admission.rs).
[`bridge/transport.rs`](../../../crates/bridge/src/transport.rs) covers fixed incomplete-frame
clocks, buffered partial frames, writer deadline/close independence, stalled delivery, and idle
shutdown. The bundled workbench browser journey proves waiting stays beneath running work and the
same invocation promotes without a duplicate row; it does not simulate operating-system load.

## C1: per-connection reporting, with no inferred trust

| Promise | Executable check |
| --- | --- |
| Two different executables can share a workspace without relabeling each other's in-flight, queued, direct, composed, failed-preparation, or refused receipts. | `tests/provenance-journey.mjs`: separate Node process, renamed executable copy, actual service receipts and original-connection equality; `queued_refusals_keep_original_evidence_after_another_peer_and_workspace_release`. |
| Reconnect creates fresh evidence; surviving connections and existing receipts retain theirs. | Provenance journey disconnect/reconnect assertions for A, B, and C, including observation time and connection IDs. |
| Windows observes the connecting process, not the service's own socket. | Provenance journey compares three actual executable basenames, including the actual MCP connector. Endpoints are different processes. Linux explicitly expects unsupported observation. |
| Raw claims, session keys, full paths, and claim controls do not enter audit or either process's diagnostics. | Provenance journey private canaries in real JSONL and both service/MCP connector diagnostics; basename-only audit assertion. |
| Reloaded history retains observed evidence but has no retained application claim. Legacy fields cannot prove the remote executable. | `workbench::history::tests::live_group_claims_stay_in_memory_while_restored_receipts_keep_observed_evidence` writes and reloads actual JSONL; surface and Chromium history restored/legacy record checks. |
| Details distinguish plural active connections, unavailable observations, and unchecked signatures without a trust badge or changed admission. | Provenance journey explicit `not_checked` and platform state; actual bundled UI immutable receipt versus later session connections, hostile HTML claim rendered as text, collapsed/open state, focus retention, and narrow layout. |

C1 signature/hash verification and C2/C3 admission remain unimplemented decisions. These tests must
not be described as authenticating the upstream application or containing a compromised host.

## Execution boundaries

The process journeys spawn isolated real service/connector binaries. Their browser receipts are
synthetic, and their runtime/audit/native-host directories are temporary. The workbench browser
journey runs the real UI bundle in isolated Chromium with synthetic projection events. It is not
an installed Tauri or native-host test. Its optional `GHOSTLIGHT_TEST_NO_SANDBOX=1` is only for
an isolated CI environment whose Chromium sandbox cannot initialize; normal runs keep the sandbox.

Run process journeys against a fresh workspace build selected by `GHOSTLIGHT_BIN_DIR`; otherwise
their default debug directory could contain an older build. Installed-browser acceptance and
platform results are recorded separately. A Windows pass does not establish a Linux pass.
