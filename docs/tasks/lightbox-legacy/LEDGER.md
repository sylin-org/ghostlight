# lightbox-legacy batch: LEDGER

## RESUME HERE

T1 added isolated process orchestration and migrated the control/Console cluster. T2 migrated the
adapter/lifecycle cluster. T3 migrated the three hub-routing scenarios. 22 of 27 rows are DONE and
both sides pass. Resume with the five specialized policy/browser scenarios; both legacy and
Lightbox CI paths remain required.

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
| `all_open_golden::read_page_redaction_is_still_wired_at_the_chokepoint` | pending | pending | |
| `manage_web_config_api::config_api_returns_every_registered_key_in_registry_order` | `legacy-console-config-registry` | DONE | T1 |
| `manage_web_config_api::config_api_is_refused_when_inbound_web_from_denies_the_source` | `legacy-console-config-source-denied` | DONE | T1 |
| `hot_reload::org_policy_hot_swap_end_to_end` | pending | pending | |
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
| `manifest_validation::org_policy_file_with_config_boots_the_server` | pending | pending | |
| `mcp_protocol::tools_call_waits_for_a_late_extension_and_notes_the_wait` | pending | pending | |
| `peer_death::native_host_rides_a_service_restart_and_exits_on_browser_eof` | `legacy-browser-relay-restart` | DONE | T2 |
| `tool_enforcement::form_fill_without_extension_fails_with_parent_audit` | pending | pending | |

Status values: `pending` | `in-progress` | `DONE` | `retired` | `BLOCKED`.
