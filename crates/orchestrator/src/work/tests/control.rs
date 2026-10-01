//! Runtime-control and session-isolation regressions at the application boundary.

use super::*;

#[test]
fn human_pause_and_stop_drain_work_without_repeated_guardrail_popups() {
    for intent in [
        ghostlight_bridge::browser::RuntimeControlIntent::Hold,
        ghostlight_bridge::browser::RuntimeControlIntent::EndSession,
    ] {
        let (executor, browser, _, workspace, audit) = fixture();
        let notices = Arc::new(Notices::default());
        executor.workbench.attach_presentation(notices.clone());
        executor.governance.apply_runtime_intent(intent);
        for _ in 0..3 {
            let result = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com","new_tab":true}),
                None,
                &CancellationToken::default(),
            );
            assert_eq!(result.effect, Effect::None);
            assert_eq!(result.status, Status::Blocked);
        }
        assert!(browser.calls().is_empty());
        assert_eq!(audit.0.lock().unwrap().len(), 3);
        assert!(
            notices.0.lock().unwrap().is_empty(),
            "human controls need no guardrail popup for every queued request"
        );
    }
}
use crate::browser::{BrowserDispatch, BrowserPort, BrowserSummary};
use crate::governance::{CapabilitySet, ReasonCode};
use crate::workbench::{
    NotificationKind, WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[test]
fn policy_explain_remains_available_without_releasing_human_controls() {
    let policy = temporary_policy("explain-no-browser-authority");
    fs::write(
        &policy,
        r#"{"schema":3,"name":"no browser grants","version":"1","grants":[]}"#,
    )
    .unwrap();
    for intent in [
        RuntimeControlIntent::StartSession,
        RuntimeControlIntent::Hold,
        RuntimeControlIntent::EndSession,
    ] {
        let (executor, browser, workspaces, workspace, audit) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
        let control = executor.governance.apply_runtime_intent(intent);
        let lease = workspaces.acquire(&workspace).unwrap();
        let result = executor.execute(
            &workspace,
            "policy_explain",
            json!({}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Succeeded, "{result:?}");
        assert_eq!(result.effect, Effect::None);
        assert!(result.repeat_safe);
        assert_eq!(result.facts["layers"].as_array().unwrap().len(), 1);
        assert!(browser.calls().is_empty());
        assert_eq!(executor.governance.runtime_state(), control);
        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].requirements(), CapabilitySet::EMPTY);
        assert_eq!(records[0].permissions.checks.len(), 1);
        let evidence = &records[0].permissions.checks[0];
        assert!(evidence.allowed);
        assert_eq!(evidence.requirements, CapabilitySet::EMPTY);
        assert!(evidence.layers.is_empty());
        assert!(!evidence.request_evaluated);
        drop(records);
        drop(lease);
        let blocked = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            None,
            &CancellationToken::default(),
        );
        assert_ne!(blocked.status, Status::Succeeded);
        assert!(browser.calls().is_empty());
    }
    fs::remove_file(policy).unwrap();
}

#[test]
fn policy_explain_still_honors_cancellation_and_original_deadlines() {
    let (executor, browser, _, workspace, audit) = fixture();
    let cancellation = CancellationToken::default();
    cancellation.cancel();
    let result = executor.execute(&workspace, "policy_explain", json!({}), None, &cancellation);
    assert_eq!(result.status, Status::Cancelled);
    assert_eq!(result.facts["reason"], "cancelled");
    let mut expired = crate::work::PreparedInvocation::new("policy_explain", json!({}), None, None);
    expired.deadline = Instant::now() - Duration::from_millis(1);
    let result =
        executor.execute_prepared(&workspace, &expired, &CancellationToken::default(), false);
    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.facts["reason"], "deadline");
    assert!(browser.calls().is_empty());
    assert_eq!(audit.0.lock().unwrap().len(), 2);
}

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

#[test]
fn repeated_policy_refusals_keep_permitted_work_available_in_the_same_session() {
    let policy = TestPolicy::new();
    fs::write(
        &policy.0,
        serde_json::to_vec(&json!({
            "schema":3,"name":"test authority","version":"1",
            "grants":[{"id":"permitted-site","hosts":{"allow":["example.com"]},"allowed":["read"]}]
        }))
        .unwrap(),
    )
    .unwrap();
    let (executor, browser, workspaces, one, audit) = fixture_with_governance(policy.facade());
    let two = workspaces.admit("Independent session".into(), IntakeChannel::Mcp, None);
    let notices = Arc::new(Notices::default());
    executor.workbench.attach_presentation(notices.clone());
    let refused = json!({"on_error":"continue","steps":[
        {"id":"one","tool":"browser_navigate","arguments":{"url":"https://denied.example/"}},
        {"id":"two","tool":"browser_navigate","arguments":{"url":"https://denied.example/"}},
        {"id":"three","tool":"browser_navigate","arguments":{"url":"https://denied.example/"}},
        {"id":"four","tool":"browser_navigate","arguments":{"url":"https://denied.example/"}},
        {"id":"five","tool":"browser_navigate","arguments":{"url":"https://denied.example/"}},
        {"id":"six","tool":"browser_navigate","arguments":{"url":"https://denied.example/"}}
    ]});
    let result = executor.execute(
        &one,
        "browser_flow",
        refused,
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked);
    assert!(result.facts["steps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|step| step["status"] == "blocked"));
    assert!(browser.calls().is_empty());
    for _ in 0..6 {
        let blocked = executor.execute(
            &one,
            "browser_navigate",
            json!({"url":"https://denied.example/"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(blocked.status, Status::Blocked);
        assert_eq!(blocked.effect, Effect::None);
    }
    assert_eq!(
        executor.governance.runtime_state(),
        RuntimeControlState::Active
    );
    assert!(notices
        .0
        .lock()
        .unwrap()
        .iter()
        .all(|notice| notice.kind != NotificationKind::Attention));
    // The existing policy permits navigation. No policy change or resume is needed after refusals.
    for workspace in [&one, &two] {
        browser.push(Ok(BrowserOutcome::TabOpened {
            reused: false,
            tab: tab(
                if workspace == &one { 7 } else { 8 },
                "https://example.com/",
            ),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let allowed = executor.execute(
            workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(allowed.status, Status::Succeeded, "{allowed:?}");
    }
    assert_eq!(browser.calls().len(), 2);
    assert_eq!(
        audit
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|record| record.step.is_some())
            .count(),
        6
    );
}

#[test]
fn obsolete_arguments_are_rejected_even_in_observe_mode() {
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
            assert_eq!(result.status, Status::Failed, "{tool}: {}", result.summary);
            assert!(browser.calls().is_empty());
            assert!(audit
                .0
                .lock()
                .unwrap()
                .iter()
                .all(|record| record.permissions.checks.is_empty()));
            assert!(result
                .next_steps
                .iter()
                .any(|step| step.contains("removed")));
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
    assert_eq!(
        executor.governance.runtime_state(),
        RuntimeControlState::Active
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
            command.primitive(),
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
                command.primitive(),
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
            if matches!(command.primitive(), BrowserCommand::Observe { .. }) {
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
    let record = audit.0.lock().unwrap().last().unwrap().clone();
    assert_eq!(record.repeat_safe, Some(false));
    assert_eq!(
        record.next_steps,
        vec![crate::language::control::APPLIED_BEFORE_CHECK_FAILURE]
    );
    let human = crate::workbench::HistoryItem::from(record);
    assert!(human
        .presentation
        .repeat_detail
        .contains("change was applied"));
    assert!(!human.presentation.summary.contains("did not run"));
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
