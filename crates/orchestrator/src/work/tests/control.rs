//! Runtime-control and session-isolation regressions at the application boundary.

use super::*;
use crate::browser::{BrowserDispatch, BrowserPort, BrowserSummary};
use crate::governance::ReasonCode;
use crate::workbench::{
    NotificationKind, WorkbenchFacade, WorkbenchNotification, WorkbenchPresentationError,
    WorkbenchPresentationPort,
};
use crate::workspace::AttentionReason;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Default)]
struct Notices(Mutex<Vec<WorkbenchNotification>>);

impl WorkbenchPresentationPort for Notices {
    fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
        Ok(())
    }
    fn notify(&self, notice: WorkbenchNotification) -> Result<(), WorkbenchPresentationError> {
        self.0.lock().unwrap().push(notice);
        Ok(())
    }
}

fn facade(executor: &ApplicationExecutor) -> WorkbenchFacade {
    WorkbenchFacade::new(
        executor.workbench.clone(),
        executor.workspaces.clone(),
        executor.governance.clone(),
        Arc::new(crate::browser::RelayBrowserPort::new("test".into())),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    )
}

#[test]
fn automatic_attention_is_local_reviewed_and_never_replays_work() {
    let (executor, browser, workspaces, one, audit) = fixture();
    let two = workspaces.admit("Independent session".into(), IntakeChannel::Mcp, None);
    let notices = Arc::new(Notices::default());
    executor.workbench.attach_presentation(notices.clone());
    let refused = json!({"restrict_capabilities":["action"],"on_error":"continue","steps":[
        {"id":"one","tool":"browser_tabs","arguments":{"action":"list"}},
        {"id":"two","tool":"browser_tabs","arguments":{"action":"list"}},
        {"id":"three","tool":"browser_tabs","arguments":{"action":"list"}},
        {"id":"four","tool":"browser_tabs","arguments":{"action":"list"}}
    ]});
    let first = executor.execute(
        &one,
        "browser_flow",
        refused.clone(),
        None,
        &CancellationToken::default(),
    );
    assert!(browser.calls().is_empty());
    let incident = workspaces.attention(&one).unwrap();
    assert_eq!(incident.invocation, first.invocation);
    assert_eq!(incident.reason, AttentionReason::RepeatedDenials);
    assert!(workspaces.attention(&two).is_none());
    assert_eq!(
        executor.governance.runtime_state(),
        RuntimeControlState::Active
    );
    for _ in 0..3 {
        let blocked = executor.execute(
            &one,
            "browser_tabs",
            json!({"action":"list"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(blocked.status, Status::AttentionRequired);
        assert_eq!(workspaces.attention(&one), Some(incident.clone()));
    }
    browser.push(Ok(BrowserOutcome::Tabs { tabs: vec![] }));
    assert_eq!(
        executor
            .execute(
                &two,
                "browser_tabs",
                json!({"action":"list"}),
                None,
                &CancellationToken::default()
            )
            .status,
        Status::Succeeded
    );
    assert_eq!(browser.calls().len(), 1);
    assert_eq!(
        notices
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|notice| notice.kind == NotificationKind::Attention)
            .count(),
        1
    );
    assert!(notices
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|notice| notice.title.contains("test")));
    executor
        .governance
        .apply_runtime_intent(RuntimeControlIntent::Resume);
    assert_eq!(workspaces.attention(&one), Some(incident.clone()));
    let recovery = facade(&executor);
    for (intent, word) in [
        (RuntimeControlIntent::Hold, "paused"),
        (RuntimeControlIntent::EndSession, "stopped"),
    ] {
        executor.governance.apply_runtime_intent(intent);
        let resumed = recovery.resume_session(one.as_str(), &incident.id);
        assert!(resumed.accepted);
        assert!(resumed.message.contains(word));
        assert_eq!(browser.calls().len(), 1, "recovery must not replay");
        assert!(!recovery.resume_session(one.as_str(), &incident.id).accepted);
        workspaces.require_attention(&one, incident.clone());
    }
    executor
        .governance
        .apply_runtime_intent(RuntimeControlIntent::StartSession);
    assert!(recovery.resume_session(one.as_str(), &incident.id).accepted);
    let newer = executor.execute(
        &one,
        "browser_flow",
        refused,
        None,
        &CancellationToken::default(),
    );
    assert_ne!(newer.invocation, first.invocation);
    assert!(!recovery.resume_session(one.as_str(), &incident.id).accepted);
    assert_eq!(
        workspaces.attention(&one).unwrap().invocation,
        newer.invocation
    );
    assert!(audit
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|record| record.invocation == first.invocation && record.step.is_some()));
    let latest = workspaces.attention(&one).unwrap();
    assert!(recovery.resume_session(one.as_str(), &latest.id).accepted);
    let mut same_invocation = latest.clone();
    same_invocation.id = "new_incident_same_invocation".into();
    assert!(workspaces.require_attention(&one, same_invocation.clone()));
    assert!(!recovery.resume_session(one.as_str(), &latest.id).accepted);
    assert_eq!(workspaces.attention(&one), Some(same_invocation));
    workspaces.release(&one);
    assert!(
        !recovery
            .resume_session(one.as_str(), &newer.invocation)
            .accepted
    );
    assert!(workspaces.attention(&one).is_none());
}

#[test]
fn observe_admission_still_enforces_direct_and_composed_request_limits() {
    let path = temporary_policy("h5-observe");
    fs::write(
        &path,
        r#"{"schema":3,"name":"observe","version":"1","mode":"observe","grants":[]}"#,
    )
    .unwrap();
    for (tool, mut arguments) in [
        ("browser_tabs", json!({"action":"list"})),
        (
            "browser_navigate",
            json!({"url":"https://example.com/","new_tab":true}),
        ),
        (
            "browser_flow",
            json!({"steps":[{"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}]}),
        ),
    ] {
        for restriction in [
            json!({"restrict_capabilities":["action"]}),
            json!({"restrict_hosts":["allowed.test"]}),
        ] {
            if tool == "browser_tabs" && restriction.get("restrict_hosts").is_some() {
                continue;
            }
            arguments
                .as_object_mut()
                .unwrap()
                .retain(|key, _| !key.starts_with("restrict_"));
            arguments
                .as_object_mut()
                .unwrap()
                .extend(restriction.as_object().unwrap().clone());
            let (executor, browser, _, workspace, audit) =
                fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
            let result = executor.execute(
                &workspace,
                tool,
                arguments.clone(),
                None,
                &CancellationToken::default(),
            );
            assert_eq!(result.status, Status::Blocked, "{tool}: {}", result.summary);
            assert!(browser.calls().is_empty());
            assert!(audit
                .0
                .lock()
                .unwrap()
                .iter()
                .flat_map(|record| &record.permissions.checks)
                .any(|check| !check.allowed
                    && check.request_restricted
                    && check.request_evaluated));
        }
    }
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
    for _ in 0..6 {
        browser.push(Ok(BrowserOutcome::Tabs { tabs: vec![] }));
        assert_eq!(
            executor
                .execute(
                    &workspace,
                    "browser_tabs",
                    json!({"action":"list"}),
                    None,
                    &CancellationToken::default()
                )
                .status,
            Status::Succeeded
        );
    }
    assert!(
        executor.workspaces.attention(&workspace).is_none(),
        "observations never count as enforced denials"
    );
    fs::remove_file(path).unwrap();
}

// The hook changes controls after an actual preparation receipt or at the final port boundary.
struct ControlledBrowser {
    inner: Arc<FakeBrowser>,
    governance: GovernanceFacade,
    intent: RuntimeControlIntent,
    before: bool,
}

impl BrowserPort for ControlledBrowser {
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        let prepare = matches!(
            command,
            BrowserCommand::DescribeFocused { .. } | BrowserCommand::EvaluateScript { .. }
        );
        let result = self
            .inner
            .call(browser, workspace, command, deadline, cancelled);
        if prepare && !self.before {
            self.governance.apply_runtime_intent(self.intent);
        }
        result
    }
    fn call_guarded(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        if self.before
            && matches!(
                command,
                BrowserCommand::TypeFocused { .. } | BrowserCommand::Observe { .. }
            )
        {
            self.governance.apply_runtime_intent(self.intent);
        }
        (dispatch.admit)()?;
        self.call(
            browser,
            workspace,
            command,
            dispatch.deadline,
            dispatch.cancelled,
        )
    }
    fn browsers(&self) -> Vec<BrowserSummary> {
        self.inner.browsers()
    }
}

#[test]
fn human_controls_after_preparation_prevent_the_effect_in_direct_and_composed_work() {
    for intent in [RuntimeControlIntent::Hold, RuntimeControlIntent::EndSession] {
        for before in [false, true] {
            for composed in [false, true] {
                let (mut executor, browser, _, workspace, _) = fixture();
                browser.push(Ok(BrowserOutcome::TabOpened {
                    reused: false,
                    tab: tab(7, "https://example.com/"),
                    committed_urls: vec![],
                }));
                executor.execute(
                    &workspace,
                    "browser_navigate",
                    json!({"url":"https://example.com/","new_tab":true}),
                    None,
                    &CancellationToken::default(),
                );
                executor.browser = Arc::new(ControlledBrowser {
                    inner: browser.clone(),
                    governance: executor.governance.clone(),
                    intent,
                    before,
                });
                browser.push(Ok(BrowserOutcome::TargetsDescribed {
                    tab_id: 7,
                    targets: vec![ObservedTarget {
                        locator: "textbox".into(),
                        role: "textbox".into(),
                        name: "Message".into(),
                        state: vec![],
                        credential_class: false,
                    }],
                }));
                let arguments = json!({"text":"not sent","focused":true});
                let (tool, arguments) = if composed {
                    (
                        "browser_flow",
                        json!({"on_error":"continue","steps":[
                            {"id":"type","tool":"browser_type_text","arguments":arguments},
                            {"id":"later","tool":"browser_navigate","arguments":{"url":"https://later.test/","new_tab":true}}
                        ]}),
                    )
                } else {
                    ("browser_type_text", arguments)
                };
                let result = executor.execute(
                    &workspace,
                    tool,
                    arguments,
                    None,
                    &CancellationToken::default(),
                );
                assert_eq!(result.effect, Effect::None);
                assert_eq!(result.status, Status::Blocked);
                assert!(result
                    .summary
                    .starts_with(if intent == RuntimeControlIntent::Hold {
                        crate::language::outcome::HUMAN_PAUSE_DIRECTIVE
                    } else {
                        crate::language::outcome::HUMAN_STOP_DIRECTIVE
                    }));
                assert_eq!(
                    browser.calls().len(),
                    2,
                    "only open and describe may reach the browser"
                );
                if composed {
                    assert_eq!(result.facts["steps"][1]["status"], "not_run");
                }
            }
        }
    }
}

#[test]
fn pause_before_postcondition_preserves_the_confirmed_action_and_audit_effect() {
    let (mut executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    // Unlike the preparation test, allow the input and change control only before observation.
    struct AfterAction(ControlledBrowser);
    impl BrowserPort for AfterAction {
        fn call(
            &self,
            b: &str,
            w: &str,
            command: BrowserCommand,
            d: Instant,
            c: &AtomicBool,
        ) -> Result<BrowserOutcome, BrowserError> {
            self.0.inner.call(b, w, command, d, c)
        }
        fn call_guarded(
            &self,
            b: &str,
            w: &str,
            command: BrowserCommand,
            d: BrowserDispatch<'_>,
        ) -> Result<BrowserOutcome, BrowserError> {
            if matches!(command, BrowserCommand::Observe { .. }) {
                self.0
                    .governance
                    .apply_runtime_intent(RuntimeControlIntent::Hold);
            }
            (d.admit)()?;
            self.call(b, w, command, d.deadline, d.cancelled)
        }
        fn browsers(&self) -> Vec<BrowserSummary> {
            self.0.inner.browsers()
        }
    }
    executor.browser = Arc::new(AfterAction(ControlledBrowser {
        inner: browser.clone(),
        governance: executor.governance.clone(),
        intent: RuntimeControlIntent::Hold,
        before: true,
    }));
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "textbox".into(),
            role: "textbox".into(),
            name: "Message".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
    browser.push(Ok(BrowserOutcome::Typed {
        tab: tab(7, "https://example.com/"),
        character_count: 4,
        subject: None,
        committed_urls: vec![],
    }));
    let result = executor.execute(
        &workspace,
        "browser_type_text",
        json!({"text":"sent","focused":true,"expect":{"condition":"text_present","value":"done"}}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked, "{}", result.summary);
    assert_eq!(result.effect, Effect::Applied);
    assert!(!result.repeat_safe);
    assert_eq!(result.facts["reason"], ReasonCode::RuntimeHold.as_str());
    assert_eq!(browser.calls().len(), 3);
    assert_eq!(audit.0.lock().unwrap().last().unwrap().effect, "applied");
}

#[test]
fn control_after_dispatch_preserves_applied_and_uncertain_receipts() {
    for uncertain in [false, true] {
        let (mut executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            reused: false,
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/","new_tab":true}),
            None,
            &CancellationToken::default(),
        );
        executor.browser = Arc::new(ControlledBrowser {
            inner: browser.clone(),
            governance: executor.governance.clone(),
            intent: RuntimeControlIntent::Hold,
            before: false,
        });
        browser.push(if uncertain {
            Err(BrowserError::EffectUnknown("reply unavailable".into()))
        } else {
            Ok(BrowserOutcome::ScriptEvaluated {
                tab: tab(7, "https://example.com/"),
                value: "1".into(),
                truncated: false,
                committed_urls: vec![],
            })
        });
        let result = executor.execute(
            &workspace,
            "browser_execute",
            json!({"script":"window.syntheticEffect = 1"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(
            result.effect,
            if uncertain {
                Effect::Unknown
            } else {
                Effect::Applied
            }
        );
        assert!(!result.repeat_safe);
        assert_eq!(
            executor.governance.runtime_state(),
            RuntimeControlState::Held
        );
        assert_eq!(browser.calls().len(), 2);
        assert_eq!(
            audit.0.lock().unwrap().last().unwrap().effect,
            if uncertain { "unknown" } else { "applied" }
        );
        executor
            .governance
            .apply_runtime_intent(RuntimeControlIntent::Resume);
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "Recovered".into(),
            truncated: false,
            title: "Test".into(),
            url: "https://example.com/".into(),
        }));
        let recovered = executor.execute(
            &workspace,
            "browser_read",
            json!({}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(
            recovered.status,
            Status::Succeeded,
            "late control must not strand the tab: {}",
            recovered.summary
        );
    }
}
