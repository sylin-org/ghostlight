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

/// The executor with every audit write failing, under the policy's audit requirement.
fn unsaved_history(
    require_audit: bool,
) -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    crate::workspace::WorkspaceId,
    Option<Scratch>,
) {
    let policy = require_audit.then(|| {
        let path = temporary_policy("require-audit");
        fs::write(
            &path,
            all_open_policy_with(
                r#"[{"key":"audit.availability","value":"require_audit","level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        path
    });
    let governance = GovernanceFacade::new(policy.as_ref().map(|path| path.to_path_buf()));
    let (mut executor, browser, _, workspace, _) = fixture_with_governance(governance);
    executor.audit = Arc::new(crate::audit::AuditRecorder::new(
        Arc::new(UnwritableAudit),
        executor.workbench.clone(),
    ));
    (executor, browser, workspace, policy)
}

/// ADR-0159 Decisions 1 and 2. When history cannot be saved, Keep working (the default) keeps
/// every outcome as it was and says so, and never leaks the storage error. Require audit refuses
/// the next browser work, a later flow step included, keeps the effects already made, rechecks at
/// the last moment before dispatch, and leaves `policy_explain` and the person's Pause working.
#[test]
fn an_audit_failure_keeps_the_outcome_and_require_audit_refuses_later_work() {
    // Keep working.
    let (executor, browser, workspace, _policy) = unsaved_history(false);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com"),
        committed_urls: vec!["https://example.com".into()],
    }));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded);
    assert_eq!(result.effect, Effect::Applied);
    assert!(!result.repeat_safe);
    assert_eq!(
        result.history_storage,
        crate::language::audit_health::Storage::Unconfirmed
    );
    assert!(result
        .summary
        .ends_with(crate::language::audit_health::HISTORY_UNSAVED));
    browser.push(Err(BrowserError::DisconnectedAfterDispatch));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        &CancellationToken::default(),
    );
    assert_eq!(result.effect, Effect::Unknown);
    assert_eq!(result.status, Status::Unknown);
    assert!(!result.repeat_safe);
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("PRIVATE_STORAGE_ERROR"));

    // Require audit.
    let (executor, browser, workspace, _policy) = unsaved_history(true);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com"),
        committed_urls: vec!["https://example.com".into()],
    }));
    let result = executor.execute(&workspace, "browser_flow", json!({"on_error":"continue","steps":[
            {"id":"first","tool":"browser_navigate","arguments":{"url":"https://example.com","new_tab":true}},
            {"id":"second","tool":"browser_navigate","arguments":{"url":"https://example.com","new_tab":true}},
            {"id":"third","tool":"browser_navigate","arguments":{"url":"https://example.com","new_tab":true}}
        ]}), &CancellationToken::default());
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
        &CancellationToken::default(),
    );
    assert_eq!(explain.status, Status::Succeeded);
    assert_eq!(explain.facts["audit"]["mode"], "require_audit");
    executor
        .governance
        .apply_runtime_intent(crate::browser::adapter::RuntimeControlIntent::Hold);
    assert_eq!(
        executor.governance.runtime_state(),
        crate::browser::adapter::RuntimeControlState::Held
    );
    let explain = executor.execute(
        &workspace,
        "policy_explain",
        json!({}),
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
        crate::browser::adapter::RuntimeControlState::Held
    );

    // Admitted earlier, refused at the last moment: storage that fails between admission and
    // dispatch stops the dispatch too.
    let (executor, _, workspace, _policy) = unsaved_history(true);
    let publication = executor.governance.published();
    let snapshot = publication.snapshot();
    let cancellation = CancellationToken::default();
    let context = InvocationContext {
        settlement_pending: &std::sync::atomic::AtomicBool::new(false),
        settlement_requested: false,
        provenance: &crate::provenance::ConnectionEvidence::claimed("test", IntakeChannel::Mcp),
        invocation: &crate::work::InvocationId::mint(),
        workspace: &workspace,
        requested_browser: None,
        requested_tab: None,
        requested_target: None,
        repeat_tool: None,
        gesture: crate::language::page_refusal::PageGesture::Other,
        snapshot,
        publication: &publication,
        requirements: CapabilitySet::READ,
        deadline: Instant::now() + Duration::from_secs(1),
        deadline_adjustable: false,
        cancellation: &cancellation,
        findings: &crate::work::findings::FrameFindings::default(),
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
}
