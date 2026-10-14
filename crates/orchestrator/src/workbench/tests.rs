use crate::audit::AuditRecorder;
use std::io;
use std::sync::{Arc, Mutex};

use crate::browser::adapter::PresentationActivity;

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
        Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::level4::assets::page_assets(),
        )),
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

/// A browser port whose connection and control publication a test chooses.
///
/// Browser work panics: a runtime control changes authority state and never dispatches or
/// replays a browser command.
struct ControlledBrowser {
    connected: bool,
    publish_fails: bool,
    published: Mutex<Vec<crate::browser::adapter::RuntimeControlState>>,
}

impl ControlledBrowser {
    fn new(connected: bool, publish_fails: bool) -> Arc<Self> {
        Arc::new(Self {
            connected,
            publish_fails,
            published: Mutex::default(),
        })
    }
}

impl crate::browser::BrowserPort for ControlledBrowser {
    fn call(
        &self,
        _: &str,
        _: &str,
        _: crate::browser::adapter::BrowserCommand,
        _: std::time::Instant,
        _: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::browser::adapter::BrowserOutcome, crate::browser::BrowserError> {
        panic!("a workbench control must not dispatch browser work");
    }

    fn browsers(&self) -> Vec<crate::browser::BrowserSummary> {
        if !self.connected {
            return Vec::new();
        }
        vec![crate::browser::BrowserSummary {
            id: "browser-1".into(),
            name: Some("Chrome".into()),
            adapter_version: "1.0.0".into(),
            platform: crate::browser::adapter::BrowserPlatform::Chromium,
            protocol: crate::browser::adapter::ADAPTER_PROTOCOL_MAJOR,
            attended: true,
        }]
    }

    fn publish_control_state(
        &self,
        state: crate::browser::adapter::RuntimeControlState,
    ) -> Result<(), crate::browser::BrowserError> {
        self.published.lock().unwrap().push(state);
        if self.publish_fails {
            Err(crate::browser::BrowserError::DisconnectedAfterDispatch)
        } else {
            Ok(())
        }
    }
}

fn facade_over(governance: GovernanceFacade, browser: Arc<ControlledBrowser>) -> WorkbenchFacade {
    WorkbenchFacade::new(
        WorkbenchProjection::default(),
        crate::workspace::WorkspaceStore::default(),
        governance,
        browser,
        crate::diagnostics::DiagnosticsHub::for_tests(),
    )
}

fn authority_card(snapshot: &super::WorkbenchSnapshot) -> super::DiagnosticItem {
    snapshot
        .diagnostics
        .iter()
        .find(|item| item.id == "authority")
        .cloned()
        .expect("the Status page always carries an authority card")
}

fn temporary_policy(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-workbench-{name}-{}.json",
        uuid::Uuid::new_v4()
    ))
}

const READ_ONLY_POLICY: &str = r#"{"schema":3,"name":"read","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[]}"#;

#[test]
fn pause_and_resume_apply_while_no_browser_is_connected() {
    use super::WorkbenchRuntimeIntent;
    use crate::browser::adapter::RuntimeControlState;
    use crate::language::readiness::Readiness;

    let governance = GovernanceFacade::new(None, None);
    let browser = ControlledBrowser::new(false, false);
    let facade = facade_over(governance.clone(), browser.clone());
    let before = facade.snapshot();
    assert_eq!(before.readiness.state, Readiness::NotConnected);
    assert!(
        before.readiness.invites_control,
        "the reachable authority must offer Pause without a browser"
    );

    for (intent, state) in [
        (WorkbenchRuntimeIntent::Hold, RuntimeControlState::Held),
        (WorkbenchRuntimeIntent::Resume, RuntimeControlState::Active),
        (
            WorkbenchRuntimeIntent::EndSession,
            RuntimeControlState::Ended,
        ),
        (
            WorkbenchRuntimeIntent::StartSession,
            RuntimeControlState::Active,
        ),
    ] {
        let result = facade.apply_runtime_intent(intent);
        assert!(result.accepted);
        assert_eq!(result.runtime_state, state);
        assert_eq!(
            result.message,
            crate::language::control::confirmation(state)
        );
        assert_eq!(governance.runtime_state(), state);
        assert_eq!(
            governance.runtime_decision().allowed,
            state == RuntimeControlState::Active,
            "the authority enforces {state:?} with no browser connected"
        );
        let snapshot = facade.snapshot();
        assert_eq!(snapshot.service.runtime_state, state);
        assert_eq!(snapshot.readiness.state, Readiness::NotConnected);
        assert!(snapshot.readiness.invites_control);
    }
}

#[test]
fn a_control_confirmation_states_the_authority_and_never_browser_delivery() {
    use super::WorkbenchRuntimeIntent;
    use crate::browser::adapter::RuntimeControlState;

    let expected = crate::language::control::confirmation(RuntimeControlState::Held);
    // Connected and published, connected but publication failed, and no browser at all: the
    // person hears the same thing, because the authority's state is the whole effect.
    for (connected, publish_fails) in [(true, false), (true, true), (false, false)] {
        let browser = ControlledBrowser::new(connected, publish_fails);
        let facade = facade_over(GovernanceFacade::new(None, None), browser.clone());
        let result = facade.apply_runtime_intent(WorkbenchRuntimeIntent::Hold);
        assert_eq!(result.runtime_state, RuntimeControlState::Held);
        assert_eq!(result.message, expected);
        assert_eq!(
            *browser.published.lock().unwrap(),
            vec![RuntimeControlState::Held],
            "every adapter still gets the best-effort notice"
        );
        let wire = serde_json::to_value(&result).unwrap();
        assert!(
            wire.get("browser_notified").is_none(),
            "the result makes no browser delivery claim: {wire}"
        );
    }
}

#[test]
fn an_invalid_policy_reads_as_not_ready_on_every_surface() {
    use crate::governance::effective::Situation;
    use crate::language::readiness::Readiness;

    let path = temporary_policy("invalid");
    std::fs::write(&path, "{half-written").unwrap();
    let facade = facade_over(
        GovernanceFacade::new(Some(path.clone()), None),
        ControlledBrowser::new(true, false),
    );
    let snapshot = facade.snapshot();
    assert_eq!(snapshot.readiness.state, Readiness::PolicyInvalid);
    assert_eq!(snapshot.readiness.word, Readiness::PolicyInvalid.word());
    assert_eq!(snapshot.readiness.tone, "attention");
    assert!(snapshot.readiness.detail.contains("Open Policy"));
    assert!(snapshot.readiness.invites_control);
    assert_eq!(
        authority_card(&snapshot).severity,
        super::DiagnosticSeverity::Failing
    );
    assert_eq!(
        snapshot.configuration.policy.situation,
        Situation::FailingClosed
    );
    assert_eq!(facade.policy().situation, Situation::FailingClosed);
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_failed_reload_that_keeps_the_last_valid_policy_stays_ready() {
    use crate::governance::effective::Situation;
    use crate::language::readiness::Readiness;

    let path = temporary_policy("retained");
    std::fs::write(&path, READ_ONLY_POLICY).unwrap();
    let facade = facade_over(
        GovernanceFacade::new(Some(path.clone()), None),
        ControlledBrowser::new(true, false),
    );
    let applied = facade.snapshot();
    assert_eq!(applied.readiness.state, Readiness::Ready);
    assert_eq!(
        authority_card(&applied).severity,
        super::DiagnosticSeverity::Passing
    );

    std::fs::write(&path, "{half-written").unwrap();
    let retained = facade.snapshot();
    // The last valid policy is still enforced, so nothing fails closed: the header stays Ready,
    // the Policy tab keeps its applied situation, and Status warns about the invalid edit.
    assert_eq!(retained.readiness.state, Readiness::Ready);
    assert_eq!(retained.configuration.policy.situation, Situation::UserOnly);
    assert_eq!(facade.policy().situation, Situation::UserOnly);
    assert_eq!(
        authority_card(&retained).severity,
        super::DiagnosticSeverity::Warning
    );
    let _ = std::fs::remove_file(path);
}
