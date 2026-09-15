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
