//! Synthetic handler/effect counters at the existing browser port, separate from authored language.

use super::*;
use crate::browser::{BrowserDispatch, BrowserPort, BrowserSummary};
use crate::governance::ReasonCode;
use crate::workspace::WorkspaceId;
use ghostlight_bridge::browser::documents::{
    DocumentInventory, DocumentObservation, PhysicalDocument,
};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Case {
    PreparationTimeout,
    PrefixHold,
    PrefixLanding,
    WrongTab,
    WrongKey,
    WrongScope,
    PrefixWrongTab,
    PrefixWrongScope,
    WorkspaceLost,
    ScriptFalse,
    ScriptThrow,
    ScriptReplyLost,
    ExpectationNotMet,
    ExpectationLost,
}

struct Mechanism {
    inner: Arc<FakeBrowser>,
    governance: GovernanceFacade,
    workspaces: WorkspaceStore,
    workspace: WorkspaceId,
    case: Case,
    handlers: AtomicU32,
    effects: AtomicU32,
    checks: AtomicU32,
}

impl Mechanism {
    fn apply(&self) {
        self.handlers.fetch_add(1, Ordering::SeqCst);
        self.effects.fetch_add(1, Ordering::SeqCst);
    }
}

impl BrowserPort for Mechanism {
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        if matches!(command, BrowserCommand::DescribeDocuments { .. })
            && self.case == Case::PreparationTimeout
        {
            return Err(BrowserError::DeadlineAfterDispatch);
        }
        if let BrowserCommand::InDocuments { scope, primitive } = command {
            return self
                .call(browser, workspace, *primitive, deadline, cancelled)
                .map(|result| BrowserOutcome::InDocuments {
                    observation: DocumentObservation {
                        visited: if self.case == Case::WrongScope
                            || (self.case == Case::PrefixWrongScope
                                && self.effects.load(Ordering::SeqCst) == 3)
                        {
                            vec!["unadmitted_document".into()]
                        } else {
                            scope.allowed
                        },
                        ..DocumentObservation::default()
                    },
                    result: Box::new(result),
                });
        }
        match command {
            BrowserCommand::PressKey { key, .. } => {
                self.apply();
                let count = self.effects.load(Ordering::SeqCst);
                let url = match (self.case, count) {
                    (Case::PrefixLanding, 1) => "https://example.com/after-first",
                    (Case::PrefixLanding, _) => "https://unapproved.invalid/after-second",
                    (Case::WorkspaceLost, _) => "https://example.com/after-key",
                    _ => "https://example.com/",
                };
                let navigated = matches!(self.case, Case::PrefixLanding | Case::WorkspaceLost);
                if self.case == Case::WorkspaceLost {
                    self.workspaces.release(&self.workspace);
                }
                self.inner.set_documents(
                    7,
                    DocumentInventory {
                        documents: vec![PhysicalDocument {
                            id: "document_7".into(),
                            url: url.into(),
                            parent: None,
                            supported: true,
                        }],
                        ..DocumentInventory::default()
                    },
                );
                Ok(BrowserOutcome::KeyPressed {
                    tab: tab(
                        if self.case == Case::WrongTab
                            || (self.case == Case::PrefixWrongTab && count == 3)
                        {
                            99
                        } else {
                            7
                        },
                        url,
                    ),
                    key: if self.case == Case::WrongKey {
                        "Other".into()
                    } else {
                        key
                    },
                    subject: None,
                    committed_urls: if navigated { vec![url.into()] } else { vec![] },
                })
            }
            BrowserCommand::EvaluateScript { .. } => {
                self.handlers.fetch_add(1, Ordering::SeqCst);
                if self.case != Case::ScriptFalse {
                    self.effects.fetch_add(1, Ordering::SeqCst);
                }
                match self.case {
                    Case::ScriptThrow => {
                        Err(BrowserError::ScriptException("PRIVATE_EXCEPTION".into()))
                    }
                    Case::ScriptReplyLost => Err(BrowserError::DisconnectedAfterDispatch),
                    _ => Ok(BrowserOutcome::ScriptEvaluated {
                        tab: tab(7, "https://example.com/"),
                        value: "false".into(),
                        truncated: false,
                        committed_urls: vec![],
                    }),
                }
            }
            BrowserCommand::TypeFocused { .. } => {
                self.apply();
                Ok(BrowserOutcome::Typed {
                    tab: tab(7, "https://example.com/"),
                    character_count: 4,
                    subject: None,
                    committed_urls: vec![],
                })
            }
            BrowserCommand::Observe { .. } => {
                self.checks.fetch_add(1, Ordering::SeqCst);
                if self.case == Case::ExpectationLost {
                    return Err(BrowserError::DisconnectedAfterDispatch);
                }
                Ok(BrowserOutcome::Observed {
                    tab_id: 7,
                    satisfied: false,
                    elapsed_ms: 1,
                    readiness: BrowserReadiness::Complete,
                })
            }
            other => self
                .inner
                .call(browser, workspace, other, deadline, cancelled),
        }
    }

    fn call_guarded(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        if self.case == Case::PrefixHold
            && matches!(command.primitive(), BrowserCommand::PressKey { .. })
            && self.effects.load(Ordering::SeqCst) == 2
        {
            self.governance
                .apply_runtime_intent(RuntimeControlIntent::Hold);
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

fn install_probe(
    executor: &mut ApplicationExecutor,
    browser: Arc<FakeBrowser>,
    workspaces: &WorkspaceStore,
    workspace: &WorkspaceId,
    case: Case,
) -> Arc<Mechanism> {
    let probe = Arc::new(Mechanism {
        inner: browser,
        governance: executor.governance.clone(),
        workspaces: workspaces.clone(),
        workspace: workspace.clone(),
        case,
        handlers: AtomicU32::new(0),
        effects: AtomicU32::new(0),
        checks: AtomicU32::new(0),
    });
    executor.browser = probe.clone();
    probe
}

fn open_fixture(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &WorkspaceId,
) -> String {
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    let result = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":"https://example.com/","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded);
    result.facts["tab"].as_str().unwrap().into()
}

#[test]
fn preparation_timeout_never_claims_a_requested_mutation_attempt() {
    let (mut executor, browser, workspaces, workspace, audit) = fixture();
    let handle = open_fixture(&executor, &browser, &workspace);
    let probe = install_probe(
        &mut executor,
        browser,
        &workspaces,
        &workspace,
        Case::PreparationTimeout,
    );
    let result = executor.execute(
        &workspace,
        "browser_press_key",
        json!({"tab":handle,"key":"x"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(probe.handlers.load(Ordering::SeqCst), 0);
    assert_eq!(probe.effects.load(Ordering::SeqCst), 0);
    assert_eq!(
        (result.status, result.effect),
        (Status::Failed, Effect::None)
    );
    assert_eq!(result.facts["execution_progress"]["attempted"], 0);
    assert!(result.summary.contains("during preparation"));
    let records = audit.0.lock().unwrap();
    assert_eq!(
        records.len(),
        2,
        "exactly one completion receipt for the failed request"
    );
    assert_eq!(
        records.last().unwrap().resolution.as_ref().unwrap().phase,
        crate::language::resolution::ResolutionPhase::Preparation
    );
}

#[test]
fn a_confirmed_keyboard_prefix_survives_a_later_unsent_hold() {
    let (mut executor, browser, workspaces, workspace, audit) = fixture();
    let handle = open_fixture(&executor, &browser, &workspace);
    let probe = install_probe(
        &mut executor,
        browser,
        &workspaces,
        &workspace,
        Case::PrefixHold,
    );
    let result = executor.execute(
        &workspace,
        "browser_press_key",
        json!({"tab":handle,"strokes":["x"],"repeat":4}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(probe.handlers.load(Ordering::SeqCst), 2);
    assert_eq!(probe.effects.load(Ordering::SeqCst), 2);
    assert_eq!(
        (result.status, result.effect),
        (Status::Blocked, Effect::Partial)
    );
    assert!(!result.repeat_safe);
    assert_eq!(result.facts["strokes_completed"], 2);
    assert_eq!(result.facts["execution_progress"]["expected"], 4);
    let records = audit.0.lock().unwrap();
    let receipt = records.last().unwrap();
    assert_eq!(
        receipt
            .resolution
            .as_ref()
            .unwrap()
            .progress
            .confirmed_effects,
        2
    );
    assert_eq!(receipt.reason, ReasonCode::RuntimeHold);
    assert_eq!(receipt.repeat_safe, Some(false));
    assert!(result.summary.contains("2 of 4"));
    assert!(crate::workbench::HistoryItem::from(receipt.clone())
        .presentation
        .repeat_detail
        .contains("Do not repeat"));
}

#[test]
fn each_keyboard_landing_is_governed_and_applied_before_the_next_stroke() {
    let policy = TestPolicy::new();
    let mut document: Value = serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
    document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
    fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
    let (mut executor, browser, workspaces, workspace, _) =
        fixture_with_governance(policy.facade());
    let handle = open_fixture(&executor, &browser, &workspace);
    let before = workspaces
        .acquire(&workspace)
        .unwrap()
        .select_tab(Some(&handle))
        .unwrap()
        .generation;
    let probe = install_probe(
        &mut executor,
        browser,
        &workspaces,
        &workspace,
        Case::PrefixLanding,
    );
    let result = executor.execute(
        &workspace,
        "browser_press_key",
        json!({"tab":handle,"strokes":["Enter"],"repeat":3}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(
        probe.effects.load(Ordering::SeqCst),
        2,
        "third stroke must never run on the denied landing"
    );
    assert_eq!(
        (result.status, result.effect),
        (Status::Blocked, Effect::Partial)
    );
    let selected = workspaces
        .acquire(&workspace)
        .unwrap()
        .tabs()
        .unwrap()
        .remove(0);
    assert_eq!(
        selected.generation,
        before + 1,
        "first governed landing was committed"
    );
    assert_eq!(selected.url, "https://example.com/after-first");
    assert!(selected.held);
    assert!(!result.repeat_safe);
}

#[test]
fn malformed_same_variant_identity_and_scope_do_not_confirm_effects() {
    for case in [Case::WrongTab, Case::WrongKey, Case::WrongScope] {
        let (mut executor, browser, workspaces, workspace, audit) = fixture();
        let handle = open_fixture(&executor, &browser, &workspace);
        let probe = install_probe(&mut executor, browser, &workspaces, &workspace, case);
        let result = executor.execute(
            &workspace,
            "browser_press_key",
            json!({"tab":handle,"key":"x"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(probe.handlers.load(Ordering::SeqCst), 1);
        assert_eq!(probe.effects.load(Ordering::SeqCst), 1);
        assert_eq!(
            (result.status, result.effect),
            (Status::Unknown, Effect::Unknown)
        );
        assert!(!result.repeat_safe);
        assert_eq!(result.facts["execution_progress"]["confirmed_effects"], 0);
        assert_eq!(
            audit
                .0
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .resolution
                .as_ref()
                .unwrap()
                .progress
                .confirmed_effects,
            0
        );
    }
}

#[test]
fn workspace_update_failure_retains_the_validated_browser_change() {
    let (mut executor, browser, workspaces, workspace, audit) = fixture();
    let handle = open_fixture(&executor, &browser, &workspace);
    let probe = install_probe(
        &mut executor,
        browser,
        &workspaces,
        &workspace,
        Case::WorkspaceLost,
    );
    let result = executor.execute(
        &workspace,
        "browser_press_key",
        json!({"tab":handle,"key":"Enter"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(probe.effects.load(Ordering::SeqCst), 1);
    assert_eq!(
        (result.status, result.effect),
        (Status::Failed, Effect::Applied)
    );
    assert!(!result.repeat_safe);
    assert_eq!(result.facts["execution_progress"]["confirmed_effects"], 1);
    assert_eq!(audit.0.lock().unwrap().last().unwrap().effect, "applied");
}

#[test]
fn a_later_malformed_receipt_keeps_the_validated_prefix_and_unknown_remainder() {
    for case in [Case::PrefixWrongTab, Case::PrefixWrongScope] {
        let (mut executor, browser, workspaces, workspace, audit) = fixture();
        let handle = open_fixture(&executor, &browser, &workspace);
        let probe = install_probe(&mut executor, browser, &workspaces, &workspace, case);
        let result = executor.execute(
            &workspace,
            "browser_press_key",
            json!({"tab":handle,"strokes":["x"],"repeat":4}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(probe.handlers.load(Ordering::SeqCst), 3);
        assert_eq!(probe.effects.load(Ordering::SeqCst), 3);
        assert_eq!(
            (result.status, result.effect),
            (Status::Unknown, Effect::Unknown)
        );
        assert!(!result.repeat_safe);
        assert_eq!(result.facts["execution_progress"]["attempted"], 3);
        assert_eq!(result.facts["strokes_completed"], 2);
        let records = audit.0.lock().unwrap();
        let record = records.last().unwrap();
        assert_eq!(
            record
                .resolution
                .as_ref()
                .unwrap()
                .progress
                .confirmed_effects,
            2
        );
        assert_eq!(
            record.refusal_facts,
            Some(crate::language::audit::AuditRefusal::IncompatibleReceipt)
        );
        assert!(record.summary.contains("2 of 4"));
    }
}

#[test]
fn script_false_exception_and_reply_loss_do_not_infer_intent_or_replay() {
    for (case, effects, status, effect) in [
        (Case::ScriptFalse, 0, Status::Succeeded, Effect::Applied),
        (Case::ScriptThrow, 1, Status::Unknown, Effect::Unknown),
        (Case::ScriptReplyLost, 1, Status::Unknown, Effect::Unknown),
    ] {
        let (mut executor, browser, workspaces, workspace, audit) = fixture();
        let handle = open_fixture(&executor, &browser, &workspace);
        let probe = install_probe(&mut executor, browser, &workspaces, &workspace, case);
        let result = executor.execute(
            &workspace,
            "browser_execute",
            json!({"tab":handle,"script":"false"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(probe.handlers.load(Ordering::SeqCst), 1);
        assert_eq!(probe.effects.load(Ordering::SeqCst), effects);
        assert_eq!((result.status, result.effect), (status, effect));
        assert!(!result.repeat_safe);
        assert_eq!(audit.0.lock().unwrap().len(), 2);
        let retained = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(!retained.contains("PRIVATE_EXCEPTION"));
        if case == Case::ScriptFalse {
            assert_eq!(result.facts["value"], false);
        }
        if case == Case::ScriptThrow {
            assert!(result.summary.contains("threw an exception"));
        }
        if case == Case::ScriptReplyLost {
            assert_eq!(
                audit.0.lock().unwrap().last().unwrap().refusal_facts,
                Some(crate::language::audit::AuditRefusal::ConnectionLost)
            );
            assert!(result.summary.starts_with("Connection lost"));
        }
    }
}

#[test]
fn unmet_and_unavailable_checks_keep_the_same_acknowledged_action() {
    for case in [Case::ExpectationNotMet, Case::ExpectationLost] {
        let (mut executor, browser, workspaces, workspace, audit) = fixture();
        let handle = open_fixture(&executor, &browser, &workspace);
        browser.push(Ok(BrowserOutcome::TargetsDescribed {
            tab_id: 7,
            targets: vec![ObservedTarget {
                locator: "field".into(),
                role: "textbox".into(),
                name: "PRIVATE_NAME".into(),
                state: vec![],
                credential_class: false,
            }],
        }));
        let probe = install_probe(&mut executor, browser, &workspaces, &workspace, case);
        let result = executor.execute(
            &workspace,
            "browser_type_text",
            json!({"tab":handle,"text":"sent","focused":true,
            "expect":{"condition":"text_present","value":"PRIVATE_EXPECTATION"}}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(probe.handlers.load(Ordering::SeqCst), 1);
        assert_eq!(probe.effects.load(Ordering::SeqCst), 1);
        assert_eq!(probe.checks.load(Ordering::SeqCst), 1);
        assert_eq!(
            (result.status, result.effect),
            (Status::Failed, Effect::Applied)
        );
        assert!(!result.repeat_safe);
        assert!(result.summary.starts_with("Typed 4 characters"));
        let records = audit.0.lock().unwrap();
        let record = records.last().unwrap();
        assert_eq!(record.summary, result.summary);
        let metadata = record.resolution.as_ref().unwrap();
        assert_eq!(metadata.progress.confirmed_effects, 1);
        assert_eq!(
            metadata.verification,
            if case == Case::ExpectationNotMet {
                crate::language::resolution::VerificationState::NotMet
            } else {
                crate::language::resolution::VerificationState::Unavailable
            }
        );
        assert!(!serde_json::to_string(record)
            .unwrap()
            .contains("PRIVATE_EXPECTATION"));
    }
}
