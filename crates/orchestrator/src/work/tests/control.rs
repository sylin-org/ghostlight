//! The person's controls at the application boundary: Pause and End stop the next effect and
//! never rewrite a sent one.

use super::*;

use crate::browser::adapter::{PhysicalTypeFocused, PhysicalWait};
use crate::browser::{BrowserDispatch, BrowserPort, BrowserSummary};
use crate::governance::{CapabilitySet, ReasonCode};
use crate::workbench::{
    PageAddress, WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Default)]
struct Notices(Mutex<Vec<WorkbenchNotification>>);

impl WorkbenchPresentationPort for Notices {
    fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
        Ok(())
    }
    fn notify(&self, notice: WorkbenchNotification) -> Result<(), WorkbenchPresentationError> {
        self.0.lock().unwrap().push(notice);
        Ok(())
    }
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
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        let prepare = matches!(
            command.primitive(),
            BrowserCommand::DescribeFocused { .. } | BrowserCommand::EvaluateScript { .. }
        );
        let result = self.inner.call(browser, command, deadline, cancelled);
        if prepare && !self.before {
            self.governance.apply_runtime_intent(self.intent);
        }
        result
    }
    fn call_guarded(
        &self,
        browser: &str,
        command: BrowserCommand,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        if self.before
            && matches!(
                command.primitive(),
                BrowserCommand::TypeFocused(PhysicalTypeFocused { .. })
                    | BrowserCommand::Observe(PhysicalWait { .. })
            )
        {
            self.governance.apply_runtime_intent(self.intent);
        }
        (dispatch.admit)()?;
        self.call(browser, command, dispatch.deadline, dispatch.cancelled)
    }
    fn browsers(&self) -> Vec<BrowserSummary> {
        self.inner.browsers()
    }
}

/// A typing target the fake browser describes for every row below.
fn message_box() -> ObservedTarget {
    ObservedTarget {
        locator: "textbox".into(),
        role: "textbox".into(),
        name: "Message".into(),
        state: vec![],
        credential_class: false,
        ..Default::default()
    }
}

/// A session with one tab open on example.com, whose browser is `browser` behind `port`.
fn session_with_a_tab(
    port: impl FnOnce(&ApplicationExecutor, Arc<FakeBrowser>) -> Arc<dyn BrowserPort>,
) -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    crate::workspace::WorkspaceId,
    Arc<MemoryAudit>,
) {
    let (mut executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/","new_tab":true}),
        &CancellationToken::default(),
    );
    executor.browser = port(&executor, browser.clone());
    (executor, browser, workspace, audit)
}

/// The directive a person's control puts first in every result it stops.
fn directive(intent: RuntimeControlIntent) -> &'static str {
    if intent == RuntimeControlIntent::Hold {
        crate::language::outcome::HUMAN_PAUSE_DIRECTIVE
    } else {
        crate::language::outcome::HUMAN_STOP_DIRECTIVE
    }
}

/// ACCEPTANCE Governance 12 and ADR-0185 items 4 and 5: the person's Pause or End stops the next
/// effect wherever it lands, and never rewrites an effect already sent.
///
/// - Before the call: the change reaches the adapters once, however often it is asked for; every
///   request is refused with no effect, each audited, and no guardrail notice pops up per queued
///   request; `policy_explain` still answers and changes no control.
/// - After preparation, directly and in a flow: the input is never sent, the result leads with the
///   directive, and a flow's later step reads `not_run`.
/// - Before a postcondition: the applied action stays applied in the result and the audit, with
///   its facts, the directive first and no retry suggested.
/// - After dispatch: an applied or uncertain receipt keeps its effect; after Resume the tab works.
#[test]
fn a_persons_control_stops_the_next_effect_and_never_rewrites_a_sent_one() {
    for intent in [RuntimeControlIntent::Hold, RuntimeControlIntent::EndSession] {
        // Before the call.
        let (executor, browser, _, workspace, audit) = fixture();
        let notices = Arc::new(Notices::default());
        executor.workbench.attach_presentation(notices.clone());
        executor.governance.relay_runtime_state(browser.clone());
        executor.governance.apply_runtime_intent(intent);
        executor.governance.apply_runtime_intent(intent);
        assert_eq!(
            browser.control_states(),
            [executor.governance.runtime_state()],
            "one change reaches the adapters once, however often it is asked for"
        );
        for _ in 0..3 {
            let result = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com","new_tab":true}),
                &CancellationToken::default(),
            );
            assert_eq!(result.effect, Effect::None);
            assert_eq!(result.status, Status::Blocked);
        }
        let explained = executor.execute(
            &workspace,
            "policy_explain",
            json!({}),
            &CancellationToken::default(),
        );
        assert_eq!(explained.status, Status::Succeeded, "{explained:?}");
        assert!(browser.calls().is_empty());
        {
            let records = audit.0.lock().unwrap();
            assert_eq!(records.len(), 4);
            let explain = records.last().unwrap();
            assert_eq!(
                explain.capabilities,
                CapabilitySet::EMPTY,
                "explaining needs no grant"
            );
        }
        assert!(notices.0.lock().unwrap().is_empty(), "no popup per request");
        assert_ne!(
            executor.governance.runtime_state(),
            RuntimeControlState::Active,
            "policy_explain changes no control"
        );

        // After preparation, before the input reaches the page.
        for before in [false, true] {
            for composed in [false, true] {
                let (executor, browser, workspace, _) = session_with_a_tab(|executor, inner| {
                    Arc::new(ControlledBrowser {
                        inner,
                        governance: executor.governance.clone(),
                        intent,
                        before,
                    })
                });
                browser.push(Ok(BrowserOutcome::TargetsDescribed {
                    rebound: false,
                    tab_id: 7,
                    targets: vec![message_box()],
                }));
                let typing = json!({"text":"not sent","focused":true});
                let (tool, arguments) = if composed {
                    (
                        "browser_flow",
                        json!({"on_error":"continue","steps":[
                            {"id":"type","tool":"browser_type_text","arguments":typing},
                            {"id":"later","tool":"browser_navigate","arguments":{"url":"https://later.test/","new_tab":true}}
                        ]}),
                    )
                } else {
                    ("browser_type_text", typing)
                };
                let result =
                    executor.execute(&workspace, tool, arguments, &CancellationToken::default());
                assert_eq!(result.effect, Effect::None);
                assert_eq!(result.status, Status::Blocked);
                assert!(result.summary.starts_with(directive(intent)), "{result:?}");
                assert_eq!(browser.calls().len(), 2, "only the open and the describe");
                if composed {
                    assert_eq!(result.facts["steps"][1]["status"], "not_run");
                }
            }
        }
    }

    // Before a postcondition: the typing landed, the check that would follow it never runs.
    struct BeforeTheCheck(ControlledBrowser);
    impl BrowserPort for BeforeTheCheck {
        fn call(
            &self,
            b: &str,
            command: BrowserCommand,
            d: Instant,
            c: &AtomicBool,
        ) -> Result<BrowserOutcome, BrowserError> {
            self.0.inner.call(b, command, d, c)
        }
        fn call_guarded(
            &self,
            b: &str,
            command: BrowserCommand,
            d: BrowserDispatch<'_>,
        ) -> Result<BrowserOutcome, BrowserError> {
            if matches!(
                command.primitive(),
                BrowserCommand::Observe(PhysicalWait { .. })
            ) {
                self.0
                    .governance
                    .apply_runtime_intent(RuntimeControlIntent::Hold);
            }
            (d.admit)()?;
            self.call(b, command, d.deadline, d.cancelled)
        }
        fn browsers(&self) -> Vec<BrowserSummary> {
            self.0.inner.browsers()
        }
    }
    let (executor, browser, workspace, audit) = session_with_a_tab(|executor, inner| {
        Arc::new(BeforeTheCheck(ControlledBrowser {
            inner,
            governance: executor.governance.clone(),
            intent: RuntimeControlIntent::Hold,
            before: true,
        }))
    });
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: vec![message_box()],
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
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked, "{}", result.summary);
    assert_eq!(result.effect, Effect::Applied);
    assert!(!result.repeat_safe);
    assert!(result
        .summary
        .starts_with(crate::language::outcome::HUMAN_PAUSE_DIRECTIVE));
    assert!(
        result.summary.contains("\"Message\""),
        "the applied action follows the directive: {}",
        result.summary
    );
    assert!(result.next_steps.is_empty(), "a pause suggests no retry");
    assert_eq!(result.facts["reason"], ReasonCode::RuntimeHold.as_str());
    assert_eq!(result.facts["typed"], true, "the action's facts stay");
    assert_eq!(audit.0.lock().unwrap().last().unwrap().effect, "applied");

    // After dispatch: the receipt keeps its effect, applied or uncertain, and Resume clears it.
    for uncertain in [false, true] {
        let (executor, browser, workspace, audit) = session_with_a_tab(|executor, inner| {
            Arc::new(ControlledBrowser {
                inner,
                governance: executor.governance.clone(),
                intent: RuntimeControlIntent::Hold,
                before: false,
            })
        });
        browser.push(if uncertain {
            Err(BrowserError::EffectUnknown("reply unavailable".into()))
        } else {
            Ok(BrowserOutcome::ScriptEvaluated {
                tab: tab(7, "https://example.com/"),
                value: "1".into(),
                truncated: false,
                form: crate::browser::adapter::ScriptForm::AsWritten,
                committed_urls: vec![],
            })
        });
        let result = executor.execute(
            &workspace,
            "browser_execute",
            json!({"script":"window.syntheticEffect = 1"}),
            &CancellationToken::default(),
        );
        let effect = if uncertain {
            Effect::Unknown
        } else {
            Effect::Applied
        };
        assert_eq!(result.effect, effect);
        assert!(!result.repeat_safe);
        assert_eq!(
            executor.governance.runtime_state(),
            RuntimeControlState::Held
        );
        assert_eq!(
            audit.0.lock().unwrap().last().unwrap().effect,
            effect.stored_name()
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
            &CancellationToken::default(),
        );
        assert_eq!(recovered.status, Status::Succeeded, "{}", recovered.summary);
    }
}
