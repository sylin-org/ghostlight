//! Operator attention is a shared dispatch rule, independent of caller and runtime holds.

use super::*;
use crate::browser::{BrowserDispatch, BrowserPort, BrowserSummary};
use ghostlight_bridge::browser::{BrowserAttention, BrowserAttentionReason};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Default)]
struct QuietNotices(Mutex<Vec<crate::workbench::WorkbenchNotification>>);

impl crate::workbench::WorkbenchPresentationPort for QuietNotices {
    fn reveal(&self) -> Result<(), crate::workbench::WorkbenchPresentationError> {
        Ok(())
    }
    fn notify(
        &self,
        notice: crate::workbench::WorkbenchNotification,
    ) -> Result<(), crate::workbench::WorkbenchPresentationError> {
        self.0.lock().unwrap().push(notice);
        Ok(())
    }
}

#[test]
fn default_background_refuses_direct_and_flow_focus_without_holding_or_notifying() {
    for composed in [false, true] {
        let (executor, browser, _, workspace, audit) = fixture();
        let notices = Arc::new(QuietNotices::default());
        executor.workbench.attach_presentation(notices.clone());
        browser.push(Ok(BrowserOutcome::TabOpened {
            reused: false,
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.facts["browser_attention"]["value"], "background");
        let before = browser.calls().len();
        let arguments = json!({"action":"focus","tab":opened.facts["tab"]});
        let (tool, input) = if composed {
            (
                "browser_flow",
                json!({"steps":[{"id":"focus","tool":"browser_tabs","arguments":arguments}]}),
            )
        } else {
            ("browser_tabs", arguments)
        };
        let result = executor.execute(&workspace, tool, input, None, &CancellationToken::default());
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::None);
        assert!(!result.repeat_safe);
        assert!(notices.0.lock().unwrap().is_empty());
        assert_eq!(browser.calls().len(), before);
        assert_eq!(
            executor.governance.runtime_state(),
            RuntimeControlState::Active
        );
        assert!(serde_json::to_string(&*audit.0.lock().unwrap())
            .unwrap()
            .contains("background"));
        assert!(!serde_json::to_string(&*audit.0.lock().unwrap())
            .unwrap()
            .contains("preserve-tabs"));
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![tab(7, "https://example.com/")],
        }));
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
}

struct TighteningWriter {
    browser: Arc<FakeBrowser>,
    policy: PathBuf,
}

impl BrowserPort for TighteningWriter {
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.browser
            .call(browser, workspace, command, deadline, cancelled)
    }
    fn call_guarded(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        assert_eq!(dispatch.attention, BrowserAttention::Foreground);
        (dispatch.admit)()?;
        // Model the person's policy change during actual relay writer contention.
        fs::write(
            &self.policy,
            all_open_policy_with(
                r#"[{"key":"browser.attention","value":"background","level":"mandatory"}]"#,
            ),
        )
        .unwrap();
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
        self.browser.browsers()
    }
}

#[test]
fn tightening_background_while_queued_prevents_the_original_foreground_effect() {
    for composed in [false, true] {
        let path = temporary_policy("attention-contention");
        fs::write(
            &path,
            all_open_policy_with(
                r#"[{"key":"browser.attention","value":"foreground","level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let (mut executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(path.clone()), None));
        executor.browser = Arc::new(TighteningWriter {
            browser: browser.clone(),
            policy: path.clone(),
        });
        let arguments = json!({"url":"https://example.com/","reuse":"never"});
        let (tool, input) = if composed {
            (
                "browser_flow",
                json!({"steps":[{"id":"open","tool":"browser_navigate","arguments":arguments}]}),
            )
        } else {
            ("browser_navigate", arguments)
        };
        let result = executor.execute(&workspace, tool, input, None, &CancellationToken::default());
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::None);
        assert!(!result.repeat_safe);
        assert_eq!(result.facts["browser_attention"]["value"], "background");
        assert!(browser.calls().is_empty());
        assert_eq!(
            executor.governance.runtime_state(),
            RuntimeControlState::Active
        );
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn physical_attention_refusals_preserve_the_actual_reason_without_preserve_tabs_copy() {
    for (reason, phrase) in [
        (BrowserAttentionReason::SharedWindowResize, "window size"),
        (BrowserAttentionReason::ActiveTabClose, "active tab"),
        (
            BrowserAttentionReason::NativeInput,
            "Native input was not sent",
        ),
    ] {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::AttentionProtected { reason }));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::None);
        assert!(!result.repeat_safe);
        assert!(result.summary.contains(phrase));
        assert_eq!(result.facts["attention_refusal"], json!(reason));
        let saved = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(saved.contains("browser_attention_protected"));
        assert!(!saved.contains("preserve-tabs"));
    }
}

#[test]
fn document_scoped_native_refusal_stays_blocked_without_effect_or_runtime_hold() {
    use ghostlight_bridge::browser::documents::{DocumentInventory, PhysicalDocument};

    let (executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
        &CancellationToken::default(),
    );
    browser.set_documents(
        7,
        DocumentInventory {
            documents: vec![PhysicalDocument {
                id: "top".into(),
                url: "https://example.com/".into(),
                parent: None,
                supported: true,
            }],
            ..DocumentInventory::default()
        },
    );
    browser.push(Ok(BrowserOutcome::AttentionProtected {
        reason: BrowserAttentionReason::NativeInput,
    }));
    let result = executor.execute(
        &workspace,
        "browser_press_key",
        json!({"tab":opened.facts["tab"],"key":"x"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(browser.scopes().len(), 1, "exercise the document envelope");
    assert_eq!(result.status, Status::Blocked);
    assert_eq!(result.effect, Effect::None);
    assert!(!result.repeat_safe);
    assert_eq!(result.facts["attention_refusal"], "native_input");
    assert_eq!(result.facts["coverage"]["inspected_documents"], 1);
    assert_eq!(
        executor.governance.runtime_state(),
        RuntimeControlState::Active
    );
    assert!(result.summary.contains("Native input was not sent"));
    assert!(serde_json::to_string(&*audit.0.lock().unwrap())
        .unwrap()
        .contains("browser_attention_protected"));
}
