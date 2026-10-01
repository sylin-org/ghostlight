use crate::audit::AuditRecorder;
use std::io;
use std::sync::{Arc, Mutex};

use ghostlight_bridge::browser::PresentationActivity;

use crate::events::{DenialPresentation, DomainEvent};
use crate::governance::{
    AuditRecord, AuditSink, Capability, CapabilitySet, Decision, GovernanceFacade,
};
use crate::language::outcome::Observed;

use super::{
    HistoryItem, NotificationKind, WorkbenchChange, WorkbenchEvent, WorkbenchEventSink,
    WorkbenchFacade, WorkbenchPresentationError, WorkbenchPresentationPort, WorkbenchProjection,
};

#[derive(Default)]
struct Events(Mutex<Vec<WorkbenchEvent>>);

impl WorkbenchEventSink for Events {
    fn publish(&self, event: WorkbenchEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[derive(Default)]
struct MemoryAudit(Mutex<Vec<AuditRecord>>);

impl AuditSink for MemoryAudit {
    fn record(&self, record: &AuditRecord) -> io::Result<()> {
        self.0.lock().unwrap().push(record.clone());
        Ok(())
    }
}

#[derive(Default)]
struct Notifications(Mutex<Vec<super::WorkbenchNotification>>);

impl WorkbenchPresentationPort for Notifications {
    fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
        Ok(())
    }

    fn notify(
        &self,
        notification: super::WorkbenchNotification,
    ) -> Result<(), WorkbenchPresentationError> {
        self.0.lock().unwrap().push(notification);
        Ok(())
    }
}

#[test]
fn projection_tracks_current_work_then_moves_it_to_history() {
    let projection = WorkbenchProjection::default();
    let notifications = Arc::new(Notifications::default());
    projection.attach_presentation(notifications.clone());
    projection.react(&DomainEvent::WorkWaiting {
        provenance: None,
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
    });
    assert_eq!(
        projection.operations()[0].phase,
        super::OperationPhase::Waiting
    );
    assert_eq!(
        projection.operations()[0].activity,
        "Waiting for earlier browser work"
    );
    assert!(notifications.0.lock().unwrap().is_empty());
    projection.react(&DomainEvent::WorkStarted {
        provenance: None,
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
    });
    assert_eq!(projection.operations().len(), 1);
    assert_eq!(
        projection.operations()[0].phase,
        super::OperationPhase::Running
    );

    let durable = Arc::new(MemoryAudit::default());
    let sink = AuditRecorder::new(durable.clone(), projection.clone());
    let governance = GovernanceFacade::new(None, None);
    let snapshot = governance.snapshot();
    let record = AuditRecord::now(
        "invocation_1",
        "workspace_1",
        "browser_read",
        Capability::Read,
        snapshot.id(),
        Decision::permitted(),
        "succeeded",
        "none",
        &crate::language::outcome::Outcome::TextRead {
            words: 1240,
            host: Some("example.com".into()),
        }
        .audit(),
        1200,
    )
    .with_observation(Observed {
        host: Some("example.com".into()),
        count: Some(1240),
        ..Observed::default()
    });
    assert_eq!(
        sink.record(&record),
        crate::language::audit_health::Storage::Saved
    );

    assert!(projection.operations().is_empty());
    assert_eq!(projection.history()[0].tool, "browser_read");
    assert_eq!(
        projection.history()[0].observed.host.as_deref(),
        Some("example.com"),
        "history dropped what the action was observed doing"
    );
    assert_eq!(durable.0.lock().unwrap().len(), 1);
}

#[test]
fn operation_lifetime_publishes_one_gapless_sequence() {
    let projection = WorkbenchProjection::default();
    let events = Arc::new(Events::default());
    projection.attach_events(events.clone());

    projection.react(&DomainEvent::WorkStarted {
        provenance: None,
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_fill_form".into(),
        activity: PresentationActivity::Fill,
        capabilities: Capability::Write.into(),
    });
    projection.react(&DomainEvent::WorkCompleted {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        physical_id: None,
    });
    let governance = GovernanceFacade::new(None, None);
    let authority = governance.snapshot();
    AuditRecorder::new(Arc::new(MemoryAudit::default()), projection.clone()).record(
        &AuditRecord::now(
            "invocation_1",
            "workspace_1",
            "browser_fill_form",
            Capability::Write,
            authority.id(),
            Decision::permitted(),
            "succeeded",
            "wrote",
            &crate::language::outcome::Outcome::TextRead {
                words: 3,
                host: None,
            }
            .audit(),
            1200,
        ),
    );

    let published = events.0.lock().unwrap();
    assert_eq!(
        published.iter().map(|event| event.seq).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "a surface must be able to detect a gap"
    );
    assert_eq!(projection.current_seq(), 3);
    match &published[0].change {
        WorkbenchChange::OperationStarted { operation } => {
            assert_eq!(operation.tool, "browser_fill_form");
            assert_eq!(operation.capability, "write");
        }
        other => panic!("expected a started change, got {other:?}"),
    }
    assert!(matches!(
        published[1].change,
        WorkbenchChange::OperationChanged { .. }
    ));
    assert!(matches!(
        published[2].change,
        WorkbenchChange::OperationSettled { .. }
    ));
}

#[test]
fn published_changes_stay_content_minimized() {
    let projection = WorkbenchProjection::default();
    let events = Arc::new(Events::default());
    projection.attach_events(events.clone());
    projection.react(&DomainEvent::WorkStarted {
        provenance: None,
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_execute".into(),
        activity: PresentationActivity::Script,
        capabilities: Capability::Execute.into(),
    });

    let settled = WorkbenchChange::OperationSettled {
        record: Box::new(HistoryItem::from(
            AuditRecord::now(
                "invocation_1",
                "workspace_1",
                "browser_execute",
                Capability::Execute,
                "authority_1",
                Decision::permitted(),
                "succeeded",
                "applied",
                &crate::language::outcome::Outcome::TargetClicked {
                    host: Some("example.com".into()),
                    subject: crate::language::outcome::ActionSubject::from_page(
                        "button", "Save", true,
                    ),
                }
                .audit(),
                140,
            )
            .with_observation(Observed {
                host: Some("example.com".into()),
                readiness: Some("complete".into()),
                ..Observed::default()
            }),
        )),
    };

    let published = events.0.lock().unwrap();
    for encoded in [
        serde_json::to_string(&published[0]).unwrap(),
        serde_json::to_string(&settled).unwrap(),
    ] {
        for forbidden in ["url", "selector", "content", "password"] {
            assert!(!encoded.contains(forbidden), "leaked {forbidden}");
        }
    }
    // The settled change still carries the observation the surface renders.
    assert!(serde_json::to_string(&settled)
        .unwrap()
        .contains("example.com"));
    assert!(serde_json::to_string(&settled).unwrap().contains("Save"));
}

#[test]
fn a_projection_without_a_sink_publishes_nothing_and_stays_at_zero() {
    let projection = WorkbenchProjection::default();
    projection.react(&DomainEvent::WorkStarted {
        provenance: None,
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
    });
    assert_eq!(projection.current_seq(), 0);
    assert_eq!(projection.operations().len(), 1);
}

#[test]
fn blocked_notifications_are_content_free_and_deduplicated() {
    let projection = WorkbenchProjection::default();
    let notifications = Arc::new(Notifications::default());
    projection.attach_presentation(notifications.clone());
    let event = DomainEvent::WorkBlocked {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        physical_id: Some(7),
        presentation: DenialPresentation::Guardrail,
    };
    projection.react(&event);
    projection.react(&event);

    let notifications = notifications.0.lock().unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].kind, NotificationKind::Blocked);
    let encoded = serde_json::to_string(&notifications[0]).unwrap();
    for forbidden in ["url", "selector", "content", "workspace_1", "invocation_1"] {
        assert!(!encoded.contains(forbidden));
    }
}

#[test]
fn presentation_failure_does_not_change_projection() {
    struct Failing;
    impl WorkbenchPresentationPort for Failing {
        fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
            Err(WorkbenchPresentationError::Native("injected".into()))
        }

        fn notify(
            &self,
            _notification: super::WorkbenchNotification,
        ) -> Result<(), WorkbenchPresentationError> {
            Err(WorkbenchPresentationError::Native("injected".into()))
        }
    }
    let projection = WorkbenchProjection::default();
    projection.attach_presentation(Arc::new(Failing));
    projection.react(&DomainEvent::AttentionRequired {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        physical_id: None,
    });
    projection.react(&DomainEvent::WorkStarted {
        provenance: None,
        invocation: "invocation_2".into(),
        workspace: "workspace_1".into(),
        tool: "browser_tabs".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
    });
    assert_eq!(projection.operations().len(), 1);
}

#[test]
fn a_preview_reports_recorded_work_a_candidate_policy_would_have_refused() {
    let projection = WorkbenchProjection::default();
    let audit = AuditRecorder::new(Arc::new(MemoryAudit::default()), projection.clone());
    for (invocation, tool, host, capability) in [
        ("i1", "browser_read", "example.com", Capability::Read),
        ("i2", "browser_read", "example.com", Capability::Read),
        ("i3", "browser_execute", "example.com", Capability::Execute),
        ("i4", "browser_read", "elsewhere.test", Capability::Read),
    ] {
        audit.record(
            &AuditRecord::now(
                invocation,
                "workspace_1",
                tool,
                CapabilitySet::one(capability),
                "authority",
                Decision::permitted(),
                "succeeded",
                "applied",
                &crate::language::outcome::Outcome::ScriptEvaluated { host: None }.audit(),
                1,
            )
            .with_observation(Observed {
                host: Some(host.into()),
                ..Observed::default()
            }),
        );
    }

    let facade = WorkbenchFacade::new(
        projection,
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None, None),
        Arc::new(crate::browser::RelayBrowserPort::new("epoch".into())),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );

    let preview = facade
            .preview_user_policy(
                r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
            )
            .unwrap();
    assert_eq!(preview.considered, 4);
    // Two reads on example.com survive; the page-code run and the other host do not.
    assert_eq!(preview.refused_total, 2);
    assert!(preview
        .summary
        .contains("If these rules had been applied, 2 of the last 4 recorded actions"));
    assert!(preview
        .refused
        .iter()
        .any(|entry| entry.tool == "browser_execute"));

    let unchanged = facade
            .preview_user_policy(
                r#"{"schema":3,"name":"open","version":"1","grants":[{"id":"everything","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]}"#,
            )
            .unwrap();
    assert_eq!(unchanged.refused_total, 0);
    assert!(unchanged
        .summary
        .contains("nothing in the last 4 recorded actions would have been refused"));

    // A draft with no rules refuses everything by definition. The count is arithmetically
    // right and useless, so the sentence explains the draft instead of reporting a number
    // that reads as a claim about work this machine already completed.
    let empty = facade
        .preview_user_policy(r#"{"schema":3,"name":"mine","version":"1","grants":[]}"#)
        .unwrap();
    assert_eq!(empty.refused_total, 4);
    assert!(empty.refused.is_empty());
    assert!(empty.summary.starts_with("These rules allow nothing yet"));
    assert!(!empty.summary.contains("4 of the last 4"));

    assert!(facade.preview_user_policy("not a policy").is_err());
}

fn controlled_reveal_tab(
    store: &crate::workspace::WorkspaceStore,
    browser: &str,
    physical_id: u64,
) -> (
    crate::workspace::WorkspaceId,
    crate::workspace::WorkspaceLease,
    String,
) {
    let workspace = store.admit(
        "reveal test".into(),
        ghostlight_bridge::service::IntakeChannel::Mcp,
        None,
    );
    store.pin_browser(workspace.as_str(), browser).unwrap();
    let lease = store.acquire(&workspace).unwrap();
    let tab = lease
        .add_tab(&ghostlight_bridge::browser::PhysicalTab {
            tab_id: physical_id,
            title: "Synthetic".into(),
            url: "https://example.com/".into(),
            active: false,
            readiness: ghostlight_bridge::browser::BrowserReadiness::Complete,
        })
        .unwrap();
    (workspace, lease, tab.handle.as_str().to_owned())
}

#[test]
fn operator_reveal_bypasses_held_writer_but_never_changes_authority_or_tab_ownership() {
    use crate::browser::testing::FakeBrowser;
    use crate::workspace::WorkspaceStore;
    use ghostlight_bridge::browser::{
        BrowserCommand, BrowserOutcome, RuntimeControlIntent, RuntimeControlState,
    };
    let store = WorkspaceStore::default();
    let (workspace, lease, tab) = controlled_reveal_tab(&store, "browser_owned", 41);
    let selected = lease.select_tab(Some(&tab)).unwrap();
    lease.hold_tab(&selected.handle).unwrap();
    let browser = FakeBrowser::default();
    let governance = GovernanceFacade::new(None, None);
    for (intent, expected) in [
        (RuntimeControlIntent::Hold, RuntimeControlState::Held),
        (RuntimeControlIntent::EndSession, RuntimeControlState::Ended),
    ] {
        governance.apply_runtime_intent(intent);
        browser.push(Ok(BrowserOutcome::TabFocused {
            tab_id: 41,
            active: true,
            window_focused: true,
        }));
        super::reveal_browser_tab(&browser, &store, workspace.as_str(), &tab).unwrap();
        assert_eq!(governance.runtime_state(), expected);
        assert_eq!(
            lease.select_tab(Some(&tab)).unwrap_err(),
            crate::workspace::WorkspaceError::Held
        );
        assert_eq!(store.summaries()[0].tab_count, 1);
        assert!(store.summaries()[0].leased);
    }
    assert!(browser
        .calls()
        .iter()
        .all(|command| matches!(command, BrowserCommand::FocusTab { tab_id: 41 })));
    assert_eq!(browser.routed(), vec!["browser_owned", "browser_owned"]);
    assert!(browser.control_states().is_empty());
}

#[test]
fn operator_reveal_refuses_foreign_stale_and_unknown_handles_without_dispatch() {
    use crate::browser::testing::FakeBrowser;
    let store = crate::workspace::WorkspaceStore::default();
    let (workspace, lease, tab) = controlled_reveal_tab(&store, "first_browser", 41);
    let (foreign, _other_lease, other_tab) = controlled_reveal_tab(&store, "second_browser", 42);
    let browser = FakeBrowser::default();
    for (id, handle) in [
        (workspace.as_str(), other_tab.as_str()),
        (workspace.as_str(), "tab_stale"),
        ("workspace_stale", tab.as_str()),
        (foreign.as_str(), tab.as_str()),
    ] {
        assert!(super::reveal_browser_tab(&browser, &store, id, handle).is_err());
    }
    store.apply_browser_close("first_browser", 41);
    assert!(super::reveal_browser_tab(&browser, &store, workspace.as_str(), &tab).is_err());
    drop(lease);
    assert!(browser.calls().is_empty());
}

#[test]
fn operator_reveal_requires_exact_active_and_window_receipt_and_does_not_replay_uncertainty() {
    use crate::browser::{testing::FakeBrowser, BrowserError};
    use ghostlight_bridge::browser::BrowserOutcome;
    let store = crate::workspace::WorkspaceStore::default();
    let (workspace, _lease, tab) = controlled_reveal_tab(&store, "browser_owned", 41);
    let browser = FakeBrowser::default();
    for outcome in [
        Ok(BrowserOutcome::TabFocused {
            tab_id: 99,
            active: true,
            window_focused: true,
        }),
        Ok(BrowserOutcome::TabFocused {
            tab_id: 41,
            active: false,
            window_focused: true,
        }),
        Ok(BrowserOutcome::TabFocused {
            tab_id: 41,
            active: true,
            window_focused: false,
        }),
        Err(BrowserError::DisconnectedAfterDispatch),
    ] {
        browser.push(outcome);
        assert!(super::reveal_browser_tab(&browser, &store, workspace.as_str(), &tab).is_err());
    }
    assert_eq!(
        browser.calls().len(),
        4,
        "uncertain reveal must not automatically replay"
    );
}

#[test]
fn show_tab_is_bound_to_actual_live_selection_and_never_reconstructed_from_audit() {
    let store = crate::workspace::WorkspaceStore::default();
    let (workspace, _lease, tab) = controlled_reveal_tab(&store, "browser_owned", 41);
    let projection = WorkbenchProjection::default();
    projection.react(&DomainEvent::WorkStarted {
        provenance: None,
        invocation: "reveal_evidence".into(),
        workspace: workspace.as_str().into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: CapabilitySet::READ,
    });
    projection.browser_tab("reveal_evidence", "foreign_workspace", 41, &store);
    assert_eq!(projection.operations()[0].tab, None);
    projection.browser_tab("reveal_evidence", workspace.as_str(), 41, &store);
    assert_eq!(
        projection.operations()[0].tab.as_deref(),
        Some(tab.as_str())
    );
    let record = AuditRecord::now(
        "reveal_evidence",
        workspace.as_str(),
        "browser_read",
        CapabilitySet::READ,
        "snapshot",
        Decision::permitted(),
        "succeeded",
        "none",
        &crate::language::outcome::Outcome::TextRead {
            words: 5,
            host: None,
        }
        .audit(),
        10,
    );
    projection.record(&record, crate::language::audit_health::Storage::Saved);
    assert_eq!(projection.history()[0].tab.as_deref(), Some(tab.as_str()));
    assert!(!serde_json::to_string(&record).unwrap().contains(&tab));
    let restored = WorkbenchProjection::default();
    restored.record(&record, crate::language::audit_health::Storage::Saved);
    assert_eq!(restored.history()[0].tab, None);
}

#[test]
fn operator_reveal_rechecks_exact_ownership_after_writer_contention() {
    use crate::browser::{BrowserDispatch, BrowserError, BrowserPort, BrowserSummary};
    use ghostlight_bridge::browser::{BrowserAttention, BrowserCommand, BrowserOutcome};
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;
    struct ClosingWriter(crate::workspace::WorkspaceStore);
    impl BrowserPort for ClosingWriter {
        fn call(
            &self,
            _: &str,
            _: &str,
            _: BrowserCommand,
            _: Instant,
            _: &AtomicBool,
        ) -> Result<BrowserOutcome, BrowserError> {
            panic!("stale reveal must never dispatch a physical focus");
        }
        fn call_guarded(
            &self,
            browser: &str,
            workspace: &str,
            command: BrowserCommand,
            dispatch: BrowserDispatch<'_>,
        ) -> Result<BrowserOutcome, BrowserError> {
            assert_eq!(dispatch.attention, BrowserAttention::Foreground);
            (dispatch.admit)().expect("initial exact ownership is valid");
            // Model the tab closing while this human command waits for the relay writer.
            self.0.apply_browser_close(browser, 41);
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
            vec![]
        }
    }
    let store = crate::workspace::WorkspaceStore::default();
    let (workspace, _lease, tab) = controlled_reveal_tab(&store, "browser_owned", 41);
    let error = super::reveal_browser_tab(
        &ClosingWriter(store.clone()),
        &store,
        workspace.as_str(),
        &tab,
    )
    .unwrap_err();
    assert!(error.contains("tab handle is stale"));
}

#[test]
fn each_flow_child_must_supply_its_own_show_tab_evidence() {
    use crate::language::history::{CompositionKind, StepReceipt};
    let store = crate::workspace::WorkspaceStore::default();
    let (workspace, _lease, tab) = controlled_reveal_tab(&store, "browser_owned", 41);
    let projection = WorkbenchProjection::default();
    projection.react(&DomainEvent::WorkStarted {
        provenance: None,
        invocation: "flow_tab_evidence".into(),
        workspace: workspace.as_str().into(),
        tool: "browser_flow".into(),
        activity: PresentationActivity::Quiet,
        capabilities: CapabilitySet::READ,
    });
    let phase = DomainEvent::WorkPhaseStarted {
        invocation: "flow_tab_evidence".into(),
        workspace: workspace.as_str().into(),
        physical_id: None,
        activity: PresentationActivity::Read,
    };
    projection.react(&phase);
    projection.browser_tab("flow_tab_evidence", workspace.as_str(), 41, &store);
    let mut child = AuditRecord::now(
        "flow_tab_evidence",
        workspace.as_str(),
        "browser_read",
        CapabilitySet::READ,
        "snapshot",
        Decision::permitted(),
        "succeeded",
        "none",
        &crate::language::outcome::Outcome::TextRead {
            words: 5,
            host: None,
        }
        .audit(),
        10,
    );
    child.step = Some(StepReceipt {
        parent: CompositionKind::Flow,
        position: 1,
        total: 3,
        preparation_failed: false,
    });
    projection.record(&child, crate::language::audit_health::Storage::Saved);
    assert_eq!(
        projection.history()[0].steps[0]
            .record
            .as_ref()
            .unwrap()
            .tab
            .as_deref(),
        Some(tab.as_str())
    );

    // A new tab-independent child cannot inherit the previous child's exact tab.
    projection.react(&phase);
    assert_eq!(projection.operations()[0].tab, None);
    child.tool = "browser_workspace".into();
    child.step.as_mut().unwrap().position = 2;
    projection.record(&child, crate::language::audit_health::Storage::Saved);
    let history = projection.history();
    assert_eq!(history[0].tab, None);
    assert_eq!(history[0].steps[1].record.as_ref().unwrap().tab, None);
    assert_eq!(
        history[0].steps[0].record.as_ref().unwrap().tab.as_deref(),
        Some(tab.as_str())
    );

    // Preparation failure may occur without entering a phase. It also has no selected-tab proof.
    projection.browser_tab("flow_tab_evidence", workspace.as_str(), 41, &store);
    child.step.as_mut().unwrap().position = 3;
    child.step.as_mut().unwrap().preparation_failed = true;
    child.status = "not_started".into();
    projection.record(&child, crate::language::audit_health::Storage::Saved);
    assert_eq!(projection.operations()[0].tab, None);
    assert_eq!(
        projection.history()[0].steps[2]
            .record
            .as_ref()
            .unwrap()
            .tab,
        None
    );
}
