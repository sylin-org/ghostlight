//! Audit failure and admission regressions at the shared completion/dispatch seams.

use super::*;
use crate::work::{CapabilitySet, Decision, InvocationContext, Outcome, ReasonCode};
use std::time::Instant;

struct UnwritableAudit;
impl AuditSink for UnwritableAudit {
    fn record(&self, _: &AuditRecord) -> io::Result<()> {
        Err(io::Error::other("PRIVATE_STORAGE_ERROR"))
    }
}

#[test]
fn audit_failure_preserves_success_unknown_effects_and_default_continuation() {
    let (mut executor, browser, _, workspace, _) = fixture();
    executor.audit = Arc::new(crate::audit::AuditRecorder::new(
        Arc::new(UnwritableAudit),
        executor.workbench.clone(),
    ));
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com"),
        committed_urls: vec!["https://example.com".into()],
    }));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded);
    assert_eq!(result.effect, Effect::Applied);
    assert!(!result.repeat_safe);
    assert_eq!(
        result.history_storage,
        crate::language::audit_health::Storage::Unconfirmed
    );
    assert!(result.summary.ends_with("History could not be saved."));
    browser.push(Err(BrowserError::DisconnectedAfterDispatch));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.effect, Effect::Unknown);
    assert_eq!(result.status, Status::Unknown);
    assert!(!result.repeat_safe);
    assert!(browser.calls().len() >= 2);
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("PRIVATE_STORAGE_ERROR"));
}

#[test]
fn strict_audit_stops_later_flow_work_under_continue_and_keeps_prior_effects() {
    let policy = temporary_policy("require-audit");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"audit.availability","value":"require_audit","level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let (mut executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
    executor.audit = Arc::new(crate::audit::AuditRecorder::new(
        Arc::new(UnwritableAudit),
        executor.workbench.clone(),
    ));
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com"),
        committed_urls: vec!["https://example.com".into()],
    }));
    let result = executor.execute(&workspace, "browser_flow", json!({"on_error":"continue","steps":[
            {"id":"first","tool":"browser_navigate","arguments":{"url":"https://example.com","new_tab":true}},
            {"id":"second","tool":"browser_navigate","arguments":{"url":"https://example.com","new_tab":true}},
            {"id":"third","tool":"browser_navigate","arguments":{"url":"https://example.com","new_tab":true}}
        ]}), None, &CancellationToken::default());
    assert_eq!(result.status, Status::Blocked, "{result:?}");
    assert_eq!(result.effect, Effect::Partial);
    assert_eq!(result.facts["progress"]["counts"]["succeeded"], 1);
    assert_eq!(result.facts["progress"]["counts"]["not_run"], 1);
    assert_eq!(
        result.facts["progress"]["issue"]["cause"],
        "audit_unavailable"
    );
    assert_eq!(result.facts["unconfirmed_history_steps"], 2);
    let commands = browser.calls().len();
    let later = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(later.status, Status::Blocked);
    assert_eq!(later.facts["policy_rule"], "audit_availability");
    assert!(later.facts["denial_id"].as_str().unwrap().starts_with("D-"));
    assert_eq!(browser.calls().len(), commands);
    let explain = executor.execute(
        &workspace,
        "policy_explain",
        json!({}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(explain.status, Status::Succeeded);
    assert_eq!(explain.facts["audit"]["mode"], "require_audit");
    executor
        .governance
        .apply_runtime_intent(ghostlight_bridge::browser::RuntimeControlIntent::Hold);
    assert_eq!(
        executor.governance.runtime_state(),
        ghostlight_bridge::browser::RuntimeControlState::Held
    );
    let explain = executor.execute(
        &workspace,
        "policy_explain",
        json!({"restrict_capabilities":["action"]}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(explain.status, Status::Succeeded);
    assert_eq!(explain.effect, Effect::None);
    assert_eq!(explain.facts["audit"]["mode"], "require_audit");
    assert_eq!(
        explain.history_storage,
        crate::language::audit_health::Storage::Unconfirmed
    );
    assert_eq!(browser.calls().len(), commands);
    assert_eq!(
        executor.governance.runtime_state(),
        ghostlight_bridge::browser::RuntimeControlState::Held
    );
    fs::remove_file(policy).unwrap();
}

#[test]
fn audit_health_is_rechecked_after_an_earlier_admission() {
    let policy = temporary_policy("audit-dispatch");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"audit.availability","value":"require_audit","level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let (mut executor, _, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
    executor.audit = Arc::new(crate::audit::AuditRecorder::new(
        Arc::new(UnwritableAudit),
        executor.workbench.clone(),
    ));
    let snapshot = executor
        .governance
        .snapshot(&crate::language::RequestRestrictions::default());
    let cancellation = CancellationToken::default();
    let context = InvocationContext {
        provenance: None,
        invocation: "queued",
        workspace: &workspace,
        requested_browser: None,
        snapshot: &snapshot,
        requirements: CapabilitySet::READ,
        deadline: Instant::now() + Duration::from_secs(1),
        cancellation: &cancellation,
    };
    assert!(executor.admit_dispatch(&context).is_ok());
    executor.audit.record(&AuditRecord::now(
        "other_session",
        "other",
        "browser_read",
        CapabilitySet::READ,
        snapshot.id(),
        Decision::permitted(),
        "succeeded",
        "none",
        &Outcome::TextRead {
            words: 1,
            host: None,
        }
        .audit(),
        1,
    ));
    assert!(matches!(
        executor.admit_dispatch(&context),
        Err(BrowserError::RuntimeControl(ReasonCode::AuditUnavailable))
    ));
    fs::remove_file(policy).unwrap();
}
