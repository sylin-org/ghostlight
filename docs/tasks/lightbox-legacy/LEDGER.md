# lightbox-legacy batch: LEDGER

2026-10-14: CLOSED. T1-T5 migrated, all 27 rows DONE; the legacy tests, wrappers and CI job were
retired 2026-07-14, and the harness itself with the 0.8 layer (ADR-0143). Do not resume.

## RESUME HERE

T1 added isolated process orchestration and migrated the control/Console cluster. T2 migrated the
adapter/lifecycle cluster. T3 migrated the three hub-routing scenarios. T4 migrated the three
specialized browser scenarios. T5 migrated the two injected local-policy scenarios. All 27 rows
are DONE and both sides pass. Resume by retiring the replaced ignored tests and legacy CI path,
then make Lightbox the sole process-boundary gate.

The 27 originals, two shell wrappers, and old `-- --ignored` CI job were retired after the parity
gate. The repaired Playwright `e2e-smoke` job remains as the real-extension/browser gate under
ADR-0056 Decision 4 and its 2026-07-14 amendment.

The Windows completion run exposed and fixed a redundant single-endpoint native-host probe that
could race the real named-pipe connection under load. Two consecutive 37-scenario runs passed
after the root fix.
The same 37-scenario replacement job passed in a clean Rust 1.95 Linux container.

## Status

| Legacy test | Lightbox scenario | Status | Commit / reason |
| --- | --- | --- | --- |
| `control_status::control_status_reports_no_extension_on_a_fresh_service` | `legacy-control-status` | DONE | T1 |
| `adapter_reconnect::adapter_reconnects_across_a_service_restart_without_a_client_reload` | `legacy-adapter-reconnect` | DONE | T2 |
| `adapter_reconnect::adapter_survives_a_five_second_service_gap` | `legacy-adapter-five-second-gap` | DONE | T2 |
| `adapter_override::unpinned_adapter_prefers_the_first_candidate_and_fails_over` | `legacy-adapter-candidate-failover` | DONE | T2 |
| `adapter_override::unpinned_adapter_falls_back_when_the_first_candidate_is_absent` | `legacy-adapter-candidate-fallback` | DONE | T2 |
| `hub_completion_criteria::two_real_adapters_multiplex_get_own_tab_groups_and_share_one_kill` | `legacy-hub-two-adapter-multiplex` | DONE | T3 |
| `hub_multiplex::one_kill_emits_one_audit_record_per_live_session` | `legacy-hub-kill-audit-fanout` | DONE | T3 |
| `hub_multiplex::adapter_endpoint_two_phase_wire_round_trips` | `legacy-hub-two-phase-wire` | DONE | T3 |
| `all_open_golden::read_page_redaction_is_still_wired_at_the_chokepoint` | `legacy-read-page-redaction` | DONE | T4 |
| `manage_web_config_api::config_api_returns_every_registered_key_in_registry_order` | `legacy-console-config-registry` | DONE | T1 |
| `manage_web_config_api::config_api_is_refused_when_inbound_web_from_denies_the_source` | `legacy-console-config-source-denied` | DONE | T1 |
| `hot_reload::org_policy_hot_swap_end_to_end` | `legacy-org-policy-hot-reload` | DONE | T5 |
| `hub_lifecycle::service_survives_the_spawning_adapter_exit` | `legacy-service-survives-adapter` | DONE | T2 |
| `hub_lifecycle::adapter_cannot_complete_handshake_with_an_impostor_service` | `legacy-adapter-anti-squat` | DONE | T2 |
| `manage_web_enable_remote::enable_remote_is_disabled_and_writes_nothing` | `legacy-console-enable-remote-disabled` | DONE | T1 |
| `manage_web_enable_remote::enable_remote_without_the_intent_header_is_refused_and_writes_nothing` | `legacy-console-enable-remote-csrf` | DONE | T1 |
| `manage_web_routes::console_index_page_is_served_over_a_real_http_get` | `legacy-console-index` | DONE | T1 |
| `manage_web_routes::console_css_and_js_are_served_with_correct_content_type` | `legacy-console-assets` | DONE | T1 |
| `manage_web_routes::unknown_path_under_api_v1_is_404` | `legacy-console-not-found` | DONE | T1 |
| `manage_web_routes::wrong_method_on_a_known_path_is_405` | `legacy-console-method-not-allowed` | DONE | T1 |
| `manage_web_routes::a_ws_upgrade_is_refused_by_default_because_web_ingestion_is_opt_in` | `legacy-console-websocket-default-off` | DONE | T1 |
| `manage_web_routes::a_real_ws_upgrade_succeeds_once_web_ingestion_is_enabled` | `legacy-console-websocket-opt-in` | DONE | T1 |
| `manage_web_sessions_api::sessions_api_reports_a_live_adapter_session_with_truncated_guid` | `legacy-console-live-sessions` | DONE | T1 |
| `manifest_validation::org_policy_file_with_config_boots_the_server` | `legacy-org-policy-boot` | DONE | T5 |
| `mcp_protocol::tools_call_waits_for_a_late_extension_and_notes_the_wait` | `legacy-late-extension-wait` | DONE | T4 |
| `peer_death::native_host_rides_a_service_restart_and_exits_on_browser_eof` | `legacy-browser-relay-restart` | DONE | T2 |
| `tool_enforcement::form_fill_without_extension_fails_with_parent_audit` | `legacy-form-fill-parent-audit` | DONE | T4 |

Status values: `pending` | `in-progress` | `DONE` | `retired` | `BLOCKED`.
