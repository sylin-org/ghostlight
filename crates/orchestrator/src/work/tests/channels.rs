//! A policy's channel switch refuses every request on its channel before any browser work
//! (ADR-0202 Decision 22a).

use std::fs;

use super::*;
use crate::governance::ReasonCode;
use crate::intake::session::SessionMarker;
use crate::language::audit::AuditRefusal;
use crate::language::self_check::golden::{
    check_golden, golden_text, rendered_result, CHANNEL_REFUSAL_GOLDEN,
};
use crate::provenance::ConnectionEvidence;
use crate::work::{Admission, InvocationId, PreparedInvocation};

/// An all-open policy whose user layer turns off work from MCP agents.
fn mcp_off() -> (Scratch, GovernanceFacade) {
    let path = temporary_policy("channel-mcp-off");
    fs::write(
        &path,
        all_open_policy_with(&format!(
            r#"[{{"key":"{}","value":false,"level":"mandatory"}}]"#,
            crate::governance::MCP_CHANNEL_ENABLED_KEY
        )),
    )
    .unwrap();
    let facade = GovernanceFacade::new(Some(path.to_path_buf()));
    (path, facade)
}

#[test]
fn an_mcp_request_is_refused_when_its_channel_is_off_and_nothing_reaches_the_browser() {
    let (_policy, facade) = mcp_off();
    let (executor, browser, _, workspace, audit) = fixture_with_governance(facade);
    for (tool, input) in [
        ("browser_navigate", json!({"url":"https://example.com/"})),
        ("policy_explain", json!({})),
    ] {
        let result = executor.execute(&workspace, tool, input, &CancellationToken::default());
        assert_eq!(result.status, Status::Blocked, "{tool}: {result:?}");
        assert_eq!(result.effect, Effect::None);
        assert!(result.repeat_safe);
        assert_eq!(
            result.summary,
            "Blocked: the user's Ghostlight policy turns off work from MCP agents."
        );
        assert!(result.next_steps.is_empty(), "{result:?}");
        assert_eq!(result.facts["reason"], "channel_denied");
        assert_eq!(result.facts["policy_rule"], "channel");
        assert!(result.facts["denial_id"].is_string());
    }
    assert!(browser.calls().is_empty());
    let records = audit.0.lock().unwrap();
    assert_eq!(records.len(), 2);
    for record in records.iter() {
        assert!(!record.allowed);
        assert_eq!(record.reason, ReasonCode::ChannelDenied);
        assert_eq!(record.policy_rule.as_deref(), Some("channel"));
        assert_eq!(record.policy_tier.as_deref(), Some("user"));
        assert_eq!(
            record.refusal_facts,
            Some(AuditRefusal::ChannelOff {
                channel: IntakeChannel::Mcp
            })
        );
        assert!(record
            .permissions
            .checks
            .iter()
            .any(|check| check.reason == ReasonCode::ChannelDenied && !check.allowed));
    }
}

#[test]
fn a_command_line_request_still_runs_when_only_mcp_is_off() {
    let (_policy, facade) = mcp_off();
    let (executor, browser, store, _, audit) = fixture_with_governance(facade);
    let caller = Arc::new(ConnectionEvidence::claimed(
        "ghostlight call",
        IntakeChannel::Cli,
    ));
    let workspace = store.connect(
        caller.clone(),
        Some(SessionMarker::Declared {
            key: "script".into(),
        }),
    );
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let prepared = PreparedInvocation::new(
        InvocationId::mint(),
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        workspace,
        caller,
    );
    let result = executor.execute_prepared(&prepared, Admission::Queued);
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    assert_eq!(browser.calls().len(), 1);
    let records = audit.0.lock().unwrap();
    assert!(records[0].allowed);
    assert_eq!(records[0].channel, Some(IntakeChannel::Cli));
}

#[test]
fn a_call_on_a_channel_that_is_off_matches_its_golden() {
    let (_policy, facade) = mcp_off();
    let (executor, _, _, workspace, _) = fixture_with_governance(facade);
    let arguments = json!({"action":"list"});
    let result = executor.execute(
        &workspace,
        "browser_tabs",
        arguments.clone(),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked, "{result:?}");
    check_golden(
        CHANNEL_REFUSAL_GOLDEN,
        &golden_text(&rendered_result("browser_tabs", arguments, &result)),
    );
}
