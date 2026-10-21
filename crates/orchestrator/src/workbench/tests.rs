use crate::audit::{AuditRecord, AuditRecorder};
use std::io;
use std::sync::{Arc, Mutex};

use crate::browser::adapter::PresentationActivity;

use crate::audit::AuditSink;
use crate::events::{DenialPresentation, DomainEvent};
use crate::governance::{Capability, CapabilitySet, Decision, GovernanceFacade};
use crate::language::outcome::Observed;
use crate::scratch::Scratch;

use super::{
    HistoryItem, NotificationKind, PageAddress, WorkbenchChange, WorkbenchEvent,
    WorkbenchEventSink, WorkbenchFacade, WorkbenchPresentationError, WorkbenchPresentationPort,
    WorkbenchProjection,
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
    fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
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
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
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
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    assert_eq!(projection.operations().len(), 1);
    assert_eq!(
        projection.operations()[0].phase,
        super::OperationPhase::Running
    );

    let durable = Arc::new(MemoryAudit::default());
    let sink = AuditRecorder::new(durable.clone(), projection.clone());
    let governance = GovernanceFacade::new(None);
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
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_fill_form".into(),
        activity: PresentationActivity::Fill,
        capabilities: Capability::Write.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    projection.react(&DomainEvent::WorkCompleted {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        physical_id: None,
    });
    let governance = GovernanceFacade::new(None);
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
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_execute".into(),
        activity: PresentationActivity::Script,
        capabilities: Capability::Execute.into(),
        received_at_ms: crate::audit::unix_ms(),
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
fn a_projection_without_a_sink_still_numbers_what_it_publishes() {
    let projection = WorkbenchProjection::default();
    projection.react(&DomainEvent::WorkStarted {
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    assert_eq!(projection.current_seq(), 1);
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
        fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
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
    projection.react(&DomainEvent::WorkBlocked {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        physical_id: None,
        presentation: DenialPresentation::Guardrail,
    });
    projection.react(&DomainEvent::WorkStarted {
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_2".into(),
        workspace: "workspace_1".into(),
        tool: "browser_tabs".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
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
                &crate::language::outcome::Outcome::ScriptEvaluated {
                    host: None,
                    empty_object: crate::language::outcome::EmptyObject::Absent,
                }
                .audit(),
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
        GovernanceFacade::new(None),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );

    let preview = facade
            .preview_user_policy(
                r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
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
                r#"{"schema":4,"name":"open","version":"1","grants":[{"id":"everything","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]}"#,
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
        .preview_user_policy(r#"{"schema":4,"name":"mine","version":"1","grants":[]}"#)
        .unwrap();
    assert_eq!(empty.refused_total, 4);
    assert!(empty.refused.is_empty());
    assert!(empty.summary.starts_with("These rules allow nothing yet"));
    assert!(!empty.summary.contains("4 of the last 4"));

    assert!(facade.preview_user_policy("not a policy").is_err());
}

#[test]
fn a_draft_that_turns_mcp_off_previews_every_mcp_action_refused() {
    use crate::intake::session::IntakeChannel;

    let projection = WorkbenchProjection::default();
    let audit = AuditRecorder::new(Arc::new(MemoryAudit::default()), projection.clone());
    // A line that kept no channel arrived on the default one, MCP, as simulate reads it.
    for (invocation, tool, channel) in [
        ("i1", "browser_read", Some(IntakeChannel::Mcp)),
        ("i2", "browser_click", Some(IntakeChannel::Mcp)),
        ("i3", "browser_read", Some(IntakeChannel::Cli)),
        ("i4", "browser_click", None),
    ] {
        let mut record = AuditRecord::now(
            invocation,
            "workspace_1",
            tool,
            CapabilitySet::one(Capability::Read),
            "authority",
            Decision::permitted(),
            "succeeded",
            "applied",
            &crate::language::outcome::Outcome::ScriptEvaluated {
                host: None,
                empty_object: crate::language::outcome::EmptyObject::Absent,
            }
            .audit(),
            1,
        )
        .with_observation(Observed {
            host: Some("example.com".into()),
            ..Observed::default()
        });
        record.channel = channel;
        audit.record(&record);
    }
    let facade = WorkbenchFacade::new(
        projection,
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );

    // Every site and every capability is allowed: only the channel switch can refuse.
    let preview = facade
        .preview_user_policy(
            r#"{"schema":4,"name":"no agents","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}],"config":[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"}]}"#,
        )
        .unwrap();
    assert_eq!(preview.considered, 4);
    assert_eq!(preview.refused_total, 3, "{preview:?}");
    let refused = |tool: &str| {
        preview
            .refused
            .iter()
            .find(|entry| entry.tool == tool)
            .map_or(0, |entry| entry.count)
    };
    assert_eq!(refused("browser_click"), 2, "{preview:?}");
    assert_eq!(
        refused("browser_read"),
        1,
        "the command line's read stays allowed: {preview:?}"
    );
    assert!(preview
        .summary
        .contains("If these rules had been applied, 3 of the last 4 recorded actions"));
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

fn temporary_policy(name: &str) -> Scratch {
    Scratch::file(&format!("workbench-{name}.json"))
}

const READ_ONLY_POLICY: &str = r#"{"schema":4,"name":"read","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[]}"#;

#[test]
fn pause_and_resume_apply_while_no_browser_is_connected() {
    use super::WorkbenchRuntimeIntent;
    use crate::browser::adapter::RuntimeControlState;
    use crate::language::readiness::Readiness;

    let governance = GovernanceFacade::new(None);
    let browser = ControlledBrowser::new(false, false);
    let facade = facade_over(governance.clone(), browser.clone());
    let before = facade.snapshot();
    assert_eq!(before.readiness.state, Readiness::NotConnected);
    assert!(
        before.readiness.invites_control,
        "the reachable authority must offer Pause without a browser"
    );

    // A person's pause and ended session are what the band says first, browser or not.
    for (intent, state, readiness) in [
        (
            WorkbenchRuntimeIntent::Hold,
            RuntimeControlState::Held,
            Readiness::Paused,
        ),
        (
            WorkbenchRuntimeIntent::Resume,
            RuntimeControlState::Active,
            Readiness::NotConnected,
        ),
        (
            WorkbenchRuntimeIntent::EndSession,
            RuntimeControlState::Ended,
            Readiness::SessionEnded,
        ),
        (
            WorkbenchRuntimeIntent::StartSession,
            RuntimeControlState::Active,
            Readiness::NotConnected,
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
        assert_eq!(snapshot.readiness.state, readiness);
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
        let governance = GovernanceFacade::new(None);
        governance.relay_runtime_state(browser.clone());
        let facade = facade_over(governance, browser.clone());
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
        GovernanceFacade::new(Some(path.to_path_buf())),
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
}

#[test]
fn switched_off_channels_are_said_on_the_band_and_in_the_rules_check() {
    use crate::language::readiness::Readiness;

    let path = temporary_policy("channels-off");
    for (switched, state, detail, rules) in [
        (
            r#"[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"}]"#,
            Readiness::Ready,
            "MCP agents are turned off; command-line agents can work.",
            "Your rules are valid. MCP agents are turned off. Command-line work may continue.",
        ),
        (
            r#"[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"},{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]"#,
            Readiness::TurnedOff,
            "Your policy turns off work from MCP agents and the command line.",
            "Your rules are valid. MCP agents and the command line are turned off, so no agent may work.",
        ),
    ] {
        std::fs::write(
            &path,
            format!(
                r#"{{"schema":4,"name":"mine","version":"1","grants":[{{"id":"all","hosts":{{"allow":["*"]}},"allowed":["read"]}}],"config":{switched}}}"#
            ),
        )
        .unwrap();
        let facade = facade_over(
            GovernanceFacade::new(Some(path.to_path_buf())),
            ControlledBrowser::new(true, false),
        );
        let snapshot = facade.snapshot();
        assert_eq!(snapshot.readiness.state, state);
        assert_eq!(snapshot.readiness.word, state.word());
        assert_eq!(snapshot.readiness.detail, detail);
        assert_eq!(authority_card(&snapshot).detail, rules);
    }
}

#[test]
fn a_failed_reload_that_keeps_the_last_valid_policy_stays_ready() {
    use crate::governance::effective::Situation;
    use crate::language::readiness::Readiness;

    let path = temporary_policy("retained");
    std::fs::write(&path, READ_ONLY_POLICY).unwrap();
    let governance = GovernanceFacade::new(Some(path.to_path_buf()));
    let facade = facade_over(governance.clone(), ControlledBrowser::new(true, false));
    let applied = facade.snapshot();
    assert_eq!(applied.readiness.state, Readiness::Ready);
    assert_eq!(
        authority_card(&applied).severity,
        super::DiagnosticSeverity::Passing
    );

    std::fs::write(&path, "{half-written").unwrap();
    governance.refresh_tick();
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
}

/// The changes a sink has been handed, in the order it got them.
fn heard(events: &Events) -> Vec<WorkbenchEvent> {
    events.0.lock().unwrap().clone()
}

fn failing_health(unreadable: u64) -> crate::language::audit_health::AuditHealth {
    crate::language::audit_health::AuditHealth {
        failure: Some(crate::language::audit_health::StorageFailure::Write),
        unconfirmed_receipts: 0,
        unreadable_entries: unreadable,
        history_unavailable: false,
    }
}

#[test]
fn the_sequence_advances_with_no_listener_and_every_sink_hears_every_event() {
    let projection = WorkbenchProjection::default();
    projection.audit_health_changed(failing_health(1));
    assert_eq!(
        projection.current_seq(),
        1,
        "a change nobody listened to still has a number"
    );

    let first = Arc::new(Events::default());
    let second = Arc::new(Events::default());
    projection.attach_events(first.clone());
    projection.attach_events(second.clone());
    projection.audit_health_changed(failing_health(2));
    projection.react(&DomainEvent::WorkStarted {
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    });

    let one = heard(&first);
    assert_eq!(
        one.iter().map(|event| event.seq).collect::<Vec<_>>(),
        [2, 3],
        "a sink attached later hears only what follows, numbered where the sequence stood"
    );
    assert_eq!(one, heard(&second));
}

/// A browser port that publishes a change while the workbench is listing the browsers.
struct BusyBrowser {
    projection: WorkbenchProjection,
    fired: std::sync::atomic::AtomicBool,
}

impl crate::browser::BrowserPort for BusyBrowser {
    fn call(
        &self,
        _: &str,
        _: crate::browser::adapter::BrowserCommand,
        _: std::time::Instant,
        _: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::browser::adapter::BrowserOutcome, crate::browser::BrowserError> {
        panic!("a snapshot must not dispatch browser work");
    }

    fn browsers(&self) -> Vec<crate::browser::BrowserSummary> {
        if !self.fired.swap(true, std::sync::atomic::Ordering::SeqCst) {
            self.projection.audit_health_changed(failing_health(7));
        }
        Vec::new()
    }

    fn publish_control_state(
        &self,
        _: crate::browser::adapter::RuntimeControlState,
    ) -> Result<(), crate::browser::BrowserError> {
        Ok(())
    }
}

#[test]
fn a_snapshot_is_never_newer_than_the_changes_it_claims() {
    let projection = WorkbenchProjection::default();
    let facade = WorkbenchFacade::new(
        projection.clone(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None),
        Arc::new(BusyBrowser {
            projection,
            fired: std::sync::atomic::AtomicBool::new(false),
        }),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let events = Arc::new(Events::default());
    facade.attach_events(events.clone());

    let snapshot = facade.snapshot();

    let landed = heard(&events)
        .into_iter()
        .find(|event| matches!(event.change, WorkbenchChange::AuditHealthChanged { .. }))
        .expect("the change landed while the snapshot was assembled");
    assert!(
        snapshot.seq < landed.seq,
        "a snapshot numbered {} claims a change numbered {}, which a page would then skip",
        snapshot.seq,
        landed.seq
    );
}

fn statuses(events: &[WorkbenchEvent]) -> Vec<super::parts::StatusPart> {
    events
        .iter()
        .filter_map(|event| match &event.change {
            WorkbenchChange::StatusChanged { status } => Some(status.clone()),
            _ => None,
        })
        .collect()
}

fn count_of(events: &[WorkbenchEvent], kind: &str) -> usize {
    events
        .iter()
        .filter(|event| serde_json::to_value(&event.change).unwrap()["kind"] == kind)
        .count()
}

#[test]
fn a_pause_says_where_it_came_from_and_a_repeat_says_nothing_new() {
    use super::RuntimeSource;
    use crate::browser::adapter::{RuntimeControlIntent, RuntimeControlState};
    use crate::language::workbench as words;

    let facade = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
    );
    let events = Arc::new(Events::default());
    facade.attach_events(events.clone());

    facade.apply_runtime_control(RuntimeControlIntent::Hold, RuntimeSource::Browser);
    let after_hold = heard(&events);
    assert_eq!(count_of(&after_hold, "runtime_changed"), 1);
    let note = statuses(&after_hold)
        .last()
        .and_then(|status| status.runtime_note.clone());
    assert_eq!(note.as_deref(), Some(words::PAUSED_FROM_BROWSER));
    assert_eq!(
        facade.snapshot().runtime_note.as_deref(),
        Some(words::PAUSED_FROM_BROWSER)
    );

    // The same pause again changes nothing a screen shows, and the first source stays.
    facade.apply_runtime_control(RuntimeControlIntent::Hold, RuntimeSource::Tray);
    let after_repeat = heard(&events);
    assert_eq!(count_of(&after_repeat, "runtime_changed"), 2);
    assert_eq!(
        statuses(&after_repeat).len(),
        statuses(&after_hold).len(),
        "an identical pause published a second status"
    );

    // Resuming drops the sentence, and the next pause names its own source.
    facade.apply_runtime_control(RuntimeControlIntent::Resume, RuntimeSource::Window);
    let resumed = statuses(&heard(&events));
    assert_eq!(resumed.last().unwrap().runtime_note, None);
    assert_eq!(
        resumed.last().unwrap().service.runtime_state,
        RuntimeControlState::Active
    );
    for (source, sentence) in [
        (RuntimeSource::Tray, words::PAUSED_FROM_TRAY),
        (RuntimeSource::Window, words::PAUSED_IN_WINDOW),
        (RuntimeSource::Watcher, words::PAUSED_FROM_WATCHER),
    ] {
        facade.apply_runtime_control(RuntimeControlIntent::ToggleHold, source);
        let last = statuses(&heard(&events)).pop().unwrap();
        assert_eq!(last.runtime_note.as_deref(), Some(sentence));
        facade.apply_runtime_control(RuntimeControlIntent::ToggleHold, source);
    }

    // The window's own intent is the window's pause.
    facade.apply_runtime_intent(super::WorkbenchRuntimeIntent::Hold);
    assert_eq!(
        facade.snapshot().runtime_note.as_deref(),
        Some(words::PAUSED_IN_WINDOW)
    );
}

#[test]
fn work_that_only_waits_leaves_the_band_ready() {
    use crate::language::readiness::Readiness;

    let projection = WorkbenchProjection::default();
    let facade = WorkbenchFacade::new(
        projection.clone(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    for invocation in ["invocation_1", "invocation_2"] {
        projection.react(&DomainEvent::WorkWaiting {
            provenance: crate::provenance::ConnectionEvidence::claimed(
                "test",
                crate::intake::session::IntakeChannel::Mcp,
            )
            .details(),
            invocation: invocation.into(),
            workspace: "workspace_1".into(),
            tool: "browser_read".into(),
            activity: PresentationActivity::Read,
            capabilities: Capability::Read.into(),
            received_at_ms: crate::audit::unix_ms(),
        });
    }
    let snapshot = facade.snapshot();
    let phases: Vec<super::OperationPhase> = snapshot
        .operations
        .iter()
        .map(|operation| operation.phase)
        .collect();
    assert_eq!(
        phases,
        [
            super::OperationPhase::Waiting,
            super::OperationPhase::Waiting
        ],
        "0 running, 2 waiting"
    );
    assert_eq!(snapshot.readiness.state, Readiness::Ready);
}

#[test]
fn work_and_saving_health_refresh_the_status_and_connections_without_a_pull() {
    let projection = WorkbenchProjection::default();
    let facade = WorkbenchFacade::new(
        projection.clone(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let events = Arc::new(Events::default());
    facade.attach_events(events.clone());

    projection.react(&DomainEvent::WorkStarted {
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "test",
            crate::intake::session::IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    let working = statuses(&heard(&events)).pop().unwrap();
    assert_eq!(
        working.readiness.state,
        crate::language::readiness::Readiness::Working
    );

    projection.audit_health_changed(failing_health(3));
    let heard_now = heard(&events);
    let health = statuses(&heard_now).pop().unwrap();
    assert_eq!(health.audit_health, failing_health(3));
    assert!(!health.audit_notice.is_empty());

    // Nothing changed, so asking again says nothing.
    let before = heard(&events).len();
    facade.refresh_live_parts();
    facade.refresh_live_parts();
    assert_eq!(heard(&events).len(), before);
}

/// Applying, editing and removing rules each hint at the policy with the id of the authority now
/// in force, so the window's one tag per part means one thing; the refresher does not repeat a
/// hint the person's own act already gave.
#[test]
fn the_authority_id_tags_the_policy_hint() {
    let path = temporary_policy("hint");
    let governance = GovernanceFacade::owning_user_policy(path.to_path_buf());
    let facade = facade_over(governance.clone(), ControlledBrowser::new(true, false));
    let events = Arc::new(Events::default());
    facade.attach_events(events.clone());

    let tag = super::parts::policy_etag(&facade.policy());
    facade.apply_user_policy_if(READ_ONLY_POLICY, &tag).unwrap();
    let hints = |events: &[WorkbenchEvent]| -> Vec<(super::parts::HintedPart, String)> {
        events
            .iter()
            .filter_map(|event| match &event.change {
                WorkbenchChange::PartChanged { part, etag } => Some((*part, etag.clone())),
                _ => None,
            })
            .collect()
    };
    let applied = hints(&heard(&events));
    assert_eq!(
        applied,
        [(
            super::parts::HintedPart::Policy,
            governance.snapshot().id().to_owned()
        )]
    );
    assert_eq!(applied[0].1, facade.policy().id);

    // The refresher has nothing new to say about an authority the person just applied.
    facade.authority_refreshed();
    assert_eq!(hints(&heard(&events)).len(), 1);

    // A rules file edited by hand is news, and the hint names the authority that is now in force.
    std::fs::write(
        &path,
        READ_ONLY_POLICY.replace("\"name\":\"read\"", "\"name\":\"look-around\""),
    )
    .unwrap();
    governance.refresh_tick();
    facade.authority_refreshed();
    let after_edit = hints(&heard(&events));
    assert_eq!(after_edit.len(), 2);
    assert_eq!(after_edit[1].0, super::parts::HintedPart::Policy);
    assert_eq!(after_edit[1].1, governance.snapshot().id());
    facade.authority_refreshed();
    assert_eq!(hints(&heard(&events)).len(), 2);

    let tag = super::parts::policy_etag(&facade.policy());
    facade.remove_user_policy_if(&tag).unwrap();
    let removed = hints(&heard(&events));
    assert_eq!(removed.len(), 3);
    assert_eq!(removed[2].1, governance.snapshot().id());
}

/// A set of native acts that records what it was asked for and answers from a script.
#[derive(Default)]
pub(super) struct RecordingActs {
    pub(super) urls: Mutex<Vec<String>>,
    pub(super) folders: Mutex<Vec<std::path::PathBuf>>,
    pub(super) titles: Mutex<Vec<String>>,
    pub(super) chosen: Mutex<Option<std::path::PathBuf>>,
}

impl RecordingActs {
    /// How many acts of any kind were asked for.
    pub(super) fn asked(&self) -> usize {
        self.urls.lock().unwrap().len()
            + self.folders.lock().unwrap().len()
            + self.titles.lock().unwrap().len()
    }
}

impl super::WorkbenchNativeActs for RecordingActs {
    fn open_url(&self, url: &str) -> Result<(), WorkbenchPresentationError> {
        self.urls.lock().unwrap().push(url.to_owned());
        Ok(())
    }

    fn open_folder(&self, path: &std::path::Path) -> Result<(), WorkbenchPresentationError> {
        self.folders.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn pick_file(
        &self,
        title: &str,
    ) -> Result<Option<std::path::PathBuf>, WorkbenchPresentationError> {
        self.titles.lock().unwrap().push(title.to_owned());
        Ok(self.chosen.lock().unwrap().clone())
    }
}

/// A temporary home, a facade over it with a client roster of its own, and acts attached when
/// asked. The home goes when its guard drops, which comes first so it outlives the facade.
fn facade_with_acts(attach: bool) -> (Scratch, WorkbenchFacade, Arc<RecordingActs>) {
    let root = Scratch::named("acts");
    let state = root.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::birth(&root),
    )
    .with_harnesses(crate::install::HarnessRegistry::isolated(
        &root.join("clients"),
        &root.join("ghostlight"),
        &state,
    ));
    let acts = Arc::new(RecordingActs::default());
    if attach {
        facade.attach_native_acts(acts.clone());
    }
    (root, facade, acts)
}

#[test]
fn a_destination_opens_exactly_its_own_address() {
    let (_root, facade, acts) = facade_with_acts(true);
    facade
        .open_destination(super::WorkbenchDestination::Home)
        .unwrap();
    assert_eq!(
        *acts.urls.lock().unwrap(),
        [super::WorkbenchDestination::Home.url()]
    );
}

#[test]
fn every_destination_has_its_name_and_points_at_the_products_own_surfaces() {
    use super::WorkbenchDestination;

    // The names the page sends. tests/workbench-settings.test.mjs pins the page's copy: About
    // offers every one but get_chrome, which only the setup view offers.
    let names = WorkbenchDestination::all().map(WorkbenchDestination::key);
    assert_eq!(
        names,
        [
            "home",
            "demo",
            "decision_aid",
            "source",
            "install_guide",
            "governance_guide",
            "scripting_guide",
            "audit_guide",
            "trust_center",
            "decision_records",
            "licensing_guide",
            "sylin_tools",
            "get_chrome",
        ]
    );
    for destination in WorkbenchDestination::all() {
        let key = destination.key();
        assert_eq!(
            serde_json::from_value::<WorkbenchDestination>(serde_json::json!(key)).ok(),
            Some(destination),
            "{key} is not the name the orchestrator reads"
        );
    }

    // Get Chrome is Google's page; every other destination is one of the product's own.
    for destination in WorkbenchDestination::all()
        .into_iter()
        .filter(|destination| *destination != WorkbenchDestination::GetChrome)
    {
        let url = destination.url();
        assert!(
            url.starts_with("https://sylin.org/")
                || url.starts_with("https://github.com/sylin-org/"),
            "{} points outside the product's own surfaces: {url}",
            destination.key()
        );
    }

    let branches: std::collections::BTreeSet<&str> = WorkbenchDestination::all()
        .into_iter()
        .filter_map(|destination| {
            let url = destination.url();
            ["/blob/", "/tree/"].into_iter().find_map(|marker| {
                let (_, rest) = url.split_once(marker)?;
                rest.split_once('/').map(|(branch, _)| branch)
            })
        })
        .collect();
    assert_eq!(
        branches,
        std::collections::BTreeSet::from(["main"]),
        "Documentation links name the published branch"
    );
    assert!(
        WorkbenchDestination::all()
            .into_iter()
            .all(|destination| !destination.url().contains("/dev/")),
        "no documentation link names the dev branch"
    );
}

#[test]
fn every_change_the_orchestrator_publishes_carries_the_kind_the_window_reads() {
    use super::parts::{ConnectionsPart, HintedPart, StatusPart};
    use super::{OperationPhase, OperationSummary};

    // Exhaustive on purpose: a new change does not compile until it is named here and given a
    // sample below.
    let named = |change: &WorkbenchChange| match change {
        WorkbenchChange::AuditHealthChanged { .. } => "audit_health_changed",
        WorkbenchChange::DocumentCoverageChanged { .. } => "document_coverage_changed",
        WorkbenchChange::OperationStarted { .. } => "operation_started",
        WorkbenchChange::OperationChanged { .. } => "operation_changed",
        WorkbenchChange::OperationSettled { .. } => "operation_settled",
        WorkbenchChange::CompositionChanged { .. } => "composition_changed",
        WorkbenchChange::RuntimeChanged { .. } => "runtime_changed",
        WorkbenchChange::StatusChanged { .. } => "status_changed",
        WorkbenchChange::ConnectionsChanged { .. } => "connections_changed",
        WorkbenchChange::ControlsChanged { .. } => "controls_changed",
        WorkbenchChange::PartChanged { .. } => "part_changed",
    };

    let snapshot = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
    )
    .snapshot();
    let operation = |phase| OperationSummary {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: "reading".into(),
        capability: "read".into(),
        started_at_ms: Some(1),
        phase,
        provenance: None,
        agent: crate::agent::Agent::unknown(),
    };
    let record = || {
        Box::new(HistoryItem::from(AuditRecord::now(
            "invocation_1",
            "workspace_1",
            "browser_read",
            Capability::Read,
            "authority_1",
            Decision::permitted(),
            "succeeded",
            "read",
            &crate::language::outcome::Outcome::TextRead {
                words: 3,
                host: None,
            }
            .audit(),
            10,
        )))
    };
    let samples = [
        WorkbenchChange::AuditHealthChanged {
            health: crate::language::audit_health::AuditHealth::default(),
        },
        WorkbenchChange::DocumentCoverageChanged {
            details: crate::language::coverage::HumanCoverage {
                summary: "Read the page.".into(),
                explanation: "Nothing was excluded.".into(),
                invocation: "invocation_1".into(),
                coverage: crate::language::coverage::Coverage::default(),
                excluded_hosts: Vec::new(),
                proactive: false,
            },
        },
        WorkbenchChange::OperationStarted {
            operation: operation(OperationPhase::Running),
        },
        WorkbenchChange::OperationChanged {
            operation: operation(OperationPhase::Waiting),
        },
        WorkbenchChange::OperationSettled { record: record() },
        WorkbenchChange::CompositionChanged { record: record() },
        WorkbenchChange::RuntimeChanged {
            runtime_state: crate::browser::adapter::RuntimeControlState::Held,
        },
        WorkbenchChange::StatusChanged {
            status: StatusPart::of(&snapshot),
        },
        WorkbenchChange::ConnectionsChanged {
            connections: ConnectionsPart::of(&snapshot),
        },
        WorkbenchChange::ControlsChanged {
            controls: Box::new(snapshot.controls.clone()),
        },
        WorkbenchChange::PartChanged {
            part: HintedPart::Integrations,
            etag: "e1".into(),
        },
    ];

    // The kinds the window folds in. PUBLISHED in tests/workbench-store.test.mjs pins the
    // window's copy: a kind renamed here and not there reads as ignored and draws nothing.
    let kinds = [
        "audit_health_changed",
        "document_coverage_changed",
        "operation_started",
        "operation_changed",
        "operation_settled",
        "composition_changed",
        "runtime_changed",
        "status_changed",
        "connections_changed",
        "controls_changed",
        "part_changed",
    ];
    assert_eq!(samples.each_ref().map(named), kinds);
    assert_eq!(
        samples.each_ref().map(|change| {
            serde_json::to_value(change).unwrap()["kind"]
                .as_str()
                .map(str::to_owned)
        }),
        kinds.map(|kind| Some(kind.to_owned()))
    );
}

#[test]
fn every_runtime_intent_reads_from_the_name_the_page_sends() {
    use super::WorkbenchRuntimeIntent;

    // Exhaustive on purpose: a new intent does not compile until it has a name. The page's buttons
    // carry these names; tests/workbench-settings.test.mjs reads them from the markup.
    let name = |intent: WorkbenchRuntimeIntent| match intent {
        WorkbenchRuntimeIntent::Hold => "hold",
        WorkbenchRuntimeIntent::Resume => "resume",
        WorkbenchRuntimeIntent::EndSession => "end_session",
        WorkbenchRuntimeIntent::StartSession => "start_session",
    };
    for intent in [
        WorkbenchRuntimeIntent::Hold,
        WorkbenchRuntimeIntent::Resume,
        WorkbenchRuntimeIntent::EndSession,
        WorkbenchRuntimeIntent::StartSession,
    ] {
        assert_eq!(
            serde_json::from_value::<WorkbenchRuntimeIntent>(serde_json::json!(name(intent))).ok(),
            Some(intent)
        );
    }
}

#[test]
fn each_host_opens_its_own_download_page_and_an_unknown_one_opens_nothing() {
    let (_root, facade, acts) = facade_with_acts(true);
    let cline: Vec<_> = facade
        .cached_harnesses()
        .into_iter()
        .filter(|summary| summary.agent.id == "cline")
        .collect();
    let hosts: Vec<&str> = cline.iter().map(|summary| summary.id.as_str()).collect();
    assert_eq!(
        hosts,
        [
            "cline-cli",
            "cline-vscode",
            "cline-cursor",
            "cline-windsurf"
        ]
    );
    for summary in &cline {
        facade.open_harness_download(&summary.id).unwrap();
    }
    let expected: Vec<String> = cline
        .iter()
        .map(|summary| {
            summary
                .download_url
                .clone()
                .expect("every Cline host has a page")
        })
        .collect();
    assert_eq!(*acts.urls.lock().unwrap(), expected);
    assert_ne!(
        expected[0], expected[1],
        "the extension's page is not the CLI's"
    );

    let refused = facade.open_harness_download("not-a-harness").unwrap_err();
    assert!(matches!(
        refused,
        super::NativeActError::Harness(crate::install::HarnessError::UnknownHarness(_))
    ));
    let refused = facade.open_harness_download("claude-desktop").unwrap_err();
    assert!(matches!(
        refused,
        super::NativeActError::Harness(crate::install::HarnessError::NoDownload(_))
    ));
    assert_eq!(acts.urls.lock().unwrap().len(), cline.len());
}

#[test]
fn the_diagnostics_folder_opens_whether_diagnostics_are_on_or_off() {
    let (root, facade, acts) = facade_with_acts(true);
    // The folder is named even while diagnostics are off, so retained logs stay readable.
    facade.reveal_diagnostics().unwrap();
    facade.set_diagnostics(true);
    facade.reveal_diagnostics().unwrap();
    let wanted = std::path::PathBuf::from(facade.diagnostics_report().directory.unwrap());
    assert_eq!(acts.folders.lock().unwrap().last(), Some(&wanted));
    assert!(wanted.starts_with(&root));
}

#[test]
fn a_refusal_to_show_diagnostics_reads_as_the_moved_sentence() {
    assert_eq!(
        super::NativeActError::DiagnosticsOff.to_string(),
        crate::language::workbench::DIAGNOSTICS_OFF
    );
}

#[test]
fn locating_by_picking_remembers_the_file_and_a_closed_dialog_changes_nothing() {
    let (root, facade, acts) = facade_with_acts(true);
    let before = facade.cached_harnesses();
    assert!(facade.locate_harness_by_picking("goose").unwrap().is_none());
    assert_eq!(
        *acts.titles.lock().unwrap(),
        [crate::language::workbench::LOCATE_DIALOG_TITLE]
    );
    assert_eq!(facade.cached_harnesses(), before);

    let located = root.join("portable/config.yaml");
    std::fs::create_dir_all(located.parent().unwrap()).unwrap();
    std::fs::write(&located, "provider: local\n").unwrap();
    *acts.chosen.lock().unwrap() = Some(located.clone());
    let result = facade.locate_harness_by_picking("goose").unwrap().unwrap();
    assert_eq!(result.summary.config_path, located.to_string_lossy());

    *acts.chosen.lock().unwrap() = Some(root.join("portable/missing.json"));
    assert!(matches!(
        facade.locate_harness_by_picking("goose"),
        Err(super::NativeActError::Harness(_))
    ));
}

#[test]
fn without_native_acts_every_act_is_unavailable() {
    let (_root, facade, acts) = facade_with_acts(false);
    let unavailable = |result: Result<(), super::NativeActError>| {
        assert!(matches!(
            result,
            Err(super::NativeActError::Workbench(
                super::WorkbenchError::PresentationUnavailable
            ))
        ));
    };
    unavailable(facade.open_destination(super::WorkbenchDestination::Home));
    unavailable(facade.open_harness_download("cursor"));
    unavailable(facade.reveal_diagnostics());
    unavailable(facade.locate_harness_by_picking("goose").map(|_| ()));
    assert_eq!(acts.asked(), 0);
}

#[test]
fn an_address_that_is_not_secure_never_reaches_the_computer() {
    let (_root, facade, acts) = facade_with_acts(true);
    for url in [
        "http://example.com/",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "",
        "HTTPS://example.com/",
    ] {
        assert!(matches!(
            facade.open_secure_link(&*acts, url),
            Err(super::NativeActError::LinkNotSecure)
        ));
    }
    assert_eq!(acts.asked(), 0);
}

#[test]
fn agents_are_counted_as_at_a_glance_groups_them_and_connections_by_intake() {
    use crate::intake::session::IntakeChannel;
    use crate::language::workbench::{service_line, ConnectionCounts};

    let workspaces = crate::workspace::WorkspaceStore::default();
    workspaces.admit("Claude Code".into(), IntakeChannel::Mcp);
    workspaces.admit("Claude Code".into(), IntakeChannel::Mcp);
    workspaces.admit("Cursor".into(), IntakeChannel::Mcp);
    workspaces.admit("a script".into(), IntakeChannel::Cli);
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    // Two Claude Code sessions are one agent, the way At a glance groups them.
    let counts = ConnectionCounts {
        agents: 3,
        mcp_clients: 3,
        command_line: 1,
        browsers: 1,
    };
    assert_eq!(facade.connection_counts(), counts);
    let snapshot = facade.snapshot();
    assert_eq!(snapshot.controls.service.word, service_line(Some(counts)));
    let grouped: std::collections::BTreeSet<&str> = snapshot
        .sessions
        .iter()
        .map(|session| session.agent.id.as_str())
        .collect();
    assert_eq!(grouped.len(), counts.agents);
    assert_eq!(
        service_line(Some(counts)),
        "Ghostlight is running; 3 agents and 1 browser connected."
    );
}

#[test]
fn the_connections_part_carries_the_counts_the_tray_says() {
    use crate::intake::session::IntakeChannel;
    use crate::language::workbench::service_line;

    let workspaces = crate::workspace::WorkspaceStore::default();
    workspaces.admit("Claude Code".into(), IntakeChannel::Mcp);
    workspaces.admit("Claude Code".into(), IntakeChannel::Mcp);
    workspaces.admit("a script".into(), IntakeChannel::Cli);
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    // The window's band and About show these numbers, so they say what the tray says.
    let part = super::parts::ConnectionsPart::of(&facade.snapshot());
    assert_eq!(part.counts, facade.connection_counts());
    assert_eq!((part.counts.agents, part.counts.browsers), (2, 1));
    assert!(facade
        .tray_model()
        .tooltip
        .ends_with(&service_line(Some(part.counts))));
    let wire = serde_json::to_value(&part).unwrap();
    assert_eq!(wire["counts"]["agents"], 2);
    assert_eq!(wire["counts"]["browsers"], 1);
}

#[test]
fn each_session_names_its_agent_as_a_person_knows_it_and_keeps_the_reported_name() {
    use crate::intake::session::IntakeChannel;

    let workspaces = crate::workspace::WorkspaceStore::default();
    workspaces.admit("codex-mcp-client".into(), IntakeChannel::Mcp);
    workspaces.admit("ghostlight call".into(), IntakeChannel::Cli);
    workspaces.admit(" my agent ".into(), IntakeChannel::Mcp);
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(false, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let mut named: Vec<(String, String, String)> = facade
        .snapshot()
        .sessions
        .into_iter()
        .map(|session| {
            let reported = session
                .connections
                .first()
                .and_then(|connection| connection.reported_application.clone())
                .unwrap_or_default();
            (session.agent.name, session.agent.id, reported)
        })
        .collect();
    named.sort();
    assert_eq!(
        named,
        [
            ("Codex".into(), "codex".into(), "codex-mcp-client".into()),
            (
                "Command line".into(),
                crate::agent::COMMAND_LINE_ID.into(),
                "ghostlight call".into()
            ),
            (
                "my agent".into(),
                "unknown:my agent".into(),
                "my agent".into()
            ),
        ]
    );
}

#[test]
fn a_session_its_work_its_history_and_its_integrations_row_carry_one_agent() {
    use crate::agent::Symbol;

    let image_of = |id: &str| Symbol::Icon {
        address: format!(
            "agent-icons/{id}?v={}",
            crate::agent::image::served(id, None).unwrap().version
        ),
        about: None,
    };
    let codex = one_agent_across_the_window("codex-mcp-client", &["codex"]);
    assert_eq!(codex.name, "Codex");
    assert_eq!(codex.symbol, image_of("codex"));
    let cline = one_agent_across_the_window(
        "cline",
        &[
            "cline-cli",
            "cline-vscode",
            "cline-cursor",
            "cline-windsurf",
        ],
    );
    assert_eq!(cline.id, "cline");
    assert_eq!(cline.name, "Cline");
    assert_eq!(cline.symbol, image_of("cline"));
}

/// The agent a session whose client calls itself `reported` carries, after showing that its
/// running work, its history item and each of the Integrations rows `hosts` carry the same one.
fn one_agent_across_the_window(reported: &str, hosts: &[&str]) -> crate::agent::Agent {
    use crate::intake::session::IntakeChannel;

    let root = Scratch::named("one-agent");
    let workspaces = crate::workspace::WorkspaceStore::default();
    let workspace = workspaces.admit(reported.into(), IntakeChannel::Mcp);
    let evidence = workspaces.single_connection(&workspace).unwrap();
    let projection = WorkbenchProjection::default();
    let facade = WorkbenchFacade::new(
        projection.clone(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(false, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    )
    .with_harnesses(crate::install::HarnessRegistry::isolated(
        &root.join("clients"),
        &root.join("ghostlight"),
        &root.join("state"),
    ));
    projection.react(&DomainEvent::WorkStarted {
        provenance: evidence.details(),
        invocation: "invocation_running".into(),
        workspace: workspace.as_str().into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    let recorder = AuditRecorder::new(Arc::new(MemoryAudit::default()), projection.clone());
    let governance = GovernanceFacade::new(None);
    let record = AuditRecord::now(
        "invocation_done",
        workspace.as_str(),
        "browser_read",
        Capability::Read,
        governance.snapshot().id(),
        Decision::permitted(),
        "succeeded",
        "none",
        &crate::language::outcome::Outcome::TextRead {
            words: 12,
            host: Some("example.com".into()),
        }
        .audit(),
        5,
    )
    .with_provenance(Some(evidence.attribution()));
    recorder.record_with_provenance(&record, Some(&evidence));
    facade.refresh_harnesses().unwrap();
    let shown = facade.snapshot();
    let agent = shown.sessions[0].agent.clone();
    assert_eq!(shown.operations[0].agent, agent);
    assert_eq!(shown.history[0].agent, agent);
    let rows: Vec<&str> = shown
        .harnesses
        .iter()
        .filter(|harness| harness.agent == agent)
        .map(|harness| harness.id.as_str())
        .collect();
    assert_eq!(rows, hosts, "every Integrations row of {reported}");
    agent
}

/// A facade over `workspaces`, with nothing else to show.
fn facade_with(
    workspaces: &crate::workspace::WorkspaceStore,
) -> (WorkbenchFacade, WorkbenchProjection) {
    let projection = WorkbenchProjection::default();
    let facade = WorkbenchFacade::new(
        projection.clone(),
        workspaces.clone(),
        GovernanceFacade::new(None),
        ControlledBrowser::new(false, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    (facade, projection)
}

#[test]
fn a_session_card_names_the_agent_its_operations_name() {
    use crate::intake::session::IntakeChannel;
    use crate::provenance::{Attribution, ConnectionEvidence, Source};

    // No name was claimed, so only the source names the agent: the card must not fall back to
    // the unknown agent while the session's running work names Claude Desktop.
    let evidence = Arc::new(ConnectionEvidence::new(
        Attribution {
            source: Some(Source {
                agent: Some("claude-desktop".into()),
                image: "claude.exe".into(),
            }),
            ..Attribution::accepted(IntakeChannel::Mcp)
        },
        None,
    ));
    let workspaces = crate::workspace::WorkspaceStore::default();
    let workspace = workspaces.connect(Arc::clone(&evidence), None);
    let (facade, projection) = facade_with(&workspaces);
    projection.react(&DomainEvent::WorkStarted {
        provenance: evidence.details(),
        invocation: "invocation_running".into(),
        workspace: workspace.as_str().into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    let shown = facade.snapshot();
    assert_eq!(shown.sessions[0].agent.name, "Claude Desktop");
    assert_eq!(shown.sessions[0].agent, shown.operations[0].agent);
    assert_eq!(facade.connection_counts().agents, 1);
}

#[test]
fn a_session_counts_only_its_running_work_as_running() {
    let evidence = Arc::new(crate::provenance::ConnectionEvidence::claimed(
        "test",
        crate::intake::session::IntakeChannel::Mcp,
    ));
    let workspaces = crate::workspace::WorkspaceStore::default();
    let workspace = workspaces.connect(Arc::clone(&evidence), None);
    let (facade, projection) = facade_with(&workspaces);
    let started = |invocation: &str| DomainEvent::WorkStarted {
        provenance: evidence.details(),
        invocation: invocation.into(),
        workspace: workspace.as_str().into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    };
    projection.react(&started("invocation_held"));
    projection.react(&DomainEvent::HoldEntered {
        invocation: "invocation_held".into(),
        workspace: workspace.as_str().into(),
        physical_id: 1,
    });
    projection.react(&DomainEvent::WorkWaiting {
        provenance: evidence.details(),
        invocation: "invocation_waiting".into(),
        workspace: workspace.as_str().into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    let shown = facade.snapshot();
    assert_eq!(shown.operations.len(), 2);
    assert_eq!(
        shown.sessions[0].running_operations, 0,
        "held and waiting work is not running"
    );

    projection.react(&started("invocation_running"));
    assert_eq!(facade.snapshot().sessions[0].running_operations, 1);
}

#[test]
fn a_command_line_session_keeps_its_name_while_no_call_is_connected() {
    use crate::intake::session::{IntakeChannel, SessionMarker};
    use crate::provenance::ConnectionEvidence;

    let call = Arc::new(ConnectionEvidence::claimed(
        "ghostlight call",
        IntakeChannel::Cli,
    ));
    let workspaces = crate::workspace::WorkspaceStore::default();
    let workspace = workspaces.connect(
        Arc::clone(&call),
        Some(SessionMarker::Declared {
            key: "script".into(),
        }),
    );
    workspaces.disconnect(
        &workspace,
        call.attribution().connection_id.as_deref().unwrap(),
    );
    let (facade, _) = facade_with(&workspaces);
    let shown = facade.snapshot();
    assert!(shown.sessions[0].connections.is_empty());
    assert_eq!(shown.sessions[0].agent, crate::agent::Agent::command_line());
    assert_eq!(shown.sessions[0].channel, IntakeChannel::Cli);
    let counts = facade.connection_counts();
    assert_eq!((counts.agents, counts.command_line), (1, 0));
}

#[test]
fn the_search_names_each_page_as_its_tab_does() {
    use crate::language::workbench as words;

    let facade = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(false, false),
    );
    let tabs = include_str!("../../ui/lib/words.js");
    for page in [
        words::SEARCH_GLANCE,
        words::SEARCH_INTEGRATIONS,
        words::SEARCH_STATUS,
        words::SEARCH_POLICY,
        words::SEARCH_SETTINGS,
    ] {
        let hit = facade
            .search(page.title)
            .into_iter()
            .find(|hit| hit.kind == super::SearchKind::Destination && hit.id == page.id)
            .expect("the search reaches every page");
        assert_eq!(hit.title, page.title);
        assert!(
            tabs.contains(&format!("{}: \"{}\"", page.id, page.title)),
            "{}",
            page.id
        );
    }
}

#[test]
fn a_session_that_reported_no_name_is_the_unknown_agent_and_its_chip_keeps_only_what_its_card_shows(
) {
    use crate::agent::Agent;
    use crate::intake::session::IntakeChannel;

    let workspaces = crate::workspace::WorkspaceStore::default();
    for unnamed in ["", "   ", "\u{200f}"] {
        workspaces.admit(unnamed.into(), IntakeChannel::Mcp);
    }
    workspaces.admit("Some\u{202e}Agent\u{200f}".into(), IntakeChannel::Mcp);
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(false, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let mut named: Vec<String> = facade
        .snapshot()
        .sessions
        .into_iter()
        .map(|session| {
            assert_eq!(session.agent.symbol, Agent::unknown().symbol);
            session.agent.name
        })
        .collect();
    named.sort();
    let unknown = crate::language::provenance::UNKNOWN_AGENT;
    assert_eq!(named, ["SomeAgent", unknown, unknown, unknown]);
    let hits = facade.search("unknown agent");
    assert_eq!(hits.len(), 3);
    assert!(hits
        .iter()
        .all(|hit| hit.title == crate::language::provenance::UNKNOWN_AGENT));
}

#[test]
fn a_service_with_nothing_connected_counts_nothing() {
    let facade = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(false, false),
    );
    assert_eq!(
        facade.connection_counts(),
        crate::language::workbench::ConnectionCounts::default()
    );
}

#[test]
fn the_controls_are_published_when_they_differ_and_not_otherwise() {
    use crate::browser::adapter::RuntimeControlState;

    let facade = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
    );
    let events = Arc::new(Events::default());
    facade.attach_events(events.clone());
    let published = || {
        events
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match &event.change {
                WorkbenchChange::ControlsChanged { controls } => Some(controls.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    facade.refresh_live_parts();
    assert_eq!(published().len(), 1, "the first look is always published");
    facade.refresh_live_parts();
    assert_eq!(published().len(), 1, "a look that found nothing new is not");

    facade.apply_runtime_intent(super::WorkbenchRuntimeIntent::Hold);
    let controls = published();
    assert_eq!(controls.len(), 2);
    assert_eq!(controls[1].runtime.state, RuntimeControlState::Held);
    assert_eq!(
        controls[1].runtime.note.as_deref(),
        Some(crate::language::workbench::PAUSED_IN_WINDOW)
    );
    assert_eq!(*controls[1], facade.snapshot().controls);
}

/// A browser port whose one browser connects and leaves when a test says so.
struct ComingAndGoing {
    connected: std::sync::atomic::AtomicBool,
}

impl ComingAndGoing {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            connected: std::sync::atomic::AtomicBool::new(false),
        })
    }

    fn connect(&self, connected: bool) {
        self.connected
            .store(connected, std::sync::atomic::Ordering::SeqCst);
    }
}

impl crate::browser::BrowserPort for ComingAndGoing {
    fn call(
        &self,
        _: &str,
        _: crate::browser::adapter::BrowserCommand,
        _: std::time::Instant,
        _: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::browser::adapter::BrowserOutcome, crate::browser::BrowserError> {
        panic!("setup must not dispatch browser work");
    }

    fn browsers(&self) -> Vec<crate::browser::BrowserSummary> {
        if !self.connected.load(std::sync::atomic::Ordering::SeqCst) {
            return Vec::new();
        }
        vec![crate::browser::BrowserSummary {
            id: "browser-1".into(),
            name: Some("Google Chrome".into()),
            adapter_version: "1.0.0".into(),
            attended: true,
        }]
    }

    fn publish_control_state(
        &self,
        _: crate::browser::adapter::RuntimeControlState,
    ) -> Result<(), crate::browser::BrowserError> {
        Ok(())
    }
}

/// A workbench that tracks an unfinished setup over a fake computer and a browser that comes and
/// goes. Its client roster is empty unless a test asks for a client, and it never reads this
/// machine's clients.
struct SetupBench {
    facade: WorkbenchFacade,
    projection: WorkbenchProjection,
    workspaces: crate::workspace::WorkspaceStore,
    browser: Arc<ComingAndGoing>,
    computer: Arc<super::setup::test_support::Computer>,
    events: Arc<Events>,
    _scratch: Scratch,
}

impl SetupBench {
    fn start() -> Self {
        Self::over(Scratch::new())
    }

    /// Like [`Self::start`], with the Ghostlight edge in place and one client found on the
    /// computer that Ghostlight can write its entry into.
    fn with_a_client_to_connect() -> Self {
        let scratch = Scratch::new();
        std::fs::write(scratch.path().join("ghostlight"), b"edge").unwrap();
        let configuration = scratch.path().join("clients/home/.cursor/mcp.json");
        std::fs::create_dir_all(configuration.parent().unwrap()).unwrap();
        std::fs::write(&configuration, b"{}").unwrap();
        Self::over(scratch)
    }

    fn over(scratch: Scratch) -> Self {
        let projection = WorkbenchProjection::default();
        let workspaces = crate::workspace::WorkspaceStore::default();
        let browser = ComingAndGoing::new();
        let computer = Arc::new(super::setup::test_support::Computer::default());
        let facade = WorkbenchFacade::new(
            projection.clone(),
            workspaces.clone(),
            GovernanceFacade::new(None),
            browser.clone(),
            crate::diagnostics::DiagnosticsHub::for_tests(),
        )
        .with_harnesses(crate::install::HarnessRegistry::isolated(
            &scratch.path().join("clients"),
            &scratch.path().join("ghostlight"),
            &scratch.path().join("state"),
        ))
        .with_setup_machine(computer.clone());
        facade.attach_setup(super::setup::SetupProgress::load(
            &scratch.path().join("state").join("setup.json"),
        ));
        let events = Arc::new(Events::default());
        facade.attach_events(events.clone());
        Self {
            facade,
            projection,
            workspaces,
            browser,
            computer,
            events,
            _scratch: scratch,
        }
    }

    /// The tags of the setup hints heard so far, in order.
    fn setup_hints(&self) -> Vec<String> {
        heard(&self.events)
            .into_iter()
            .filter_map(|event| match event.change {
                WorkbenchChange::PartChanged {
                    part: super::parts::HintedPart::Setup,
                    etag,
                } => Some(etag),
                _ => None,
            })
            .collect()
    }

    /// The tag of the setup part as a snapshot reads it now.
    fn setup_etag(&self) -> String {
        super::parts::json_etag(&self.facade.setup_view())
    }
}

fn a_request_that_reached_a_browser(workspace: &str) -> AuditRecord {
    let mut record = AuditRecord::now(
        "invocation_1",
        workspace,
        "browser_read",
        Capability::Read,
        "authority_1",
        Decision::permitted(),
        "succeeded",
        "none",
        &crate::language::outcome::Outcome::StepNotStarted.audit(),
        5,
    )
    .from_channel(Some(crate::intake::session::IntakeChannel::Mcp))
    .with_browser(Some("Google Chrome".into()));
    record.timestamp_ms = 1234;
    record
}

#[test]
fn a_service_that_tracks_no_setup_announces_none() {
    let facade = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
    );
    let events = Arc::new(Events::default());
    facade.attach_events(events.clone());
    facade.refresh_live_parts();
    facade.refresh_harnesses().unwrap();
    let setup_hints = heard(&events)
        .into_iter()
        .filter(|event| {
            matches!(
                event.change,
                WorkbenchChange::PartChanged {
                    part: super::parts::HintedPart::Setup,
                    ..
                }
            )
        })
        .count();
    assert_eq!(setup_hints, 0);
    assert_eq!(
        facade.setup_view().stage,
        crate::language::setup::Stage::Retired
    );
}

#[test]
fn a_snapshot_does_not_read_this_computer_for_the_setup_part() {
    let bench = SetupBench::start();
    bench.computer.install("chrome");
    let reads = bench.computer.reads();
    let _ = bench.facade.snapshot();
    assert_eq!(bench.computer.reads(), reads);
    assert_eq!(
        bench.facade.setup_view().stage,
        crate::language::setup::Stage::Browser
    );
    assert!(bench.computer.reads() > reads, "the setup view reads it");
}

#[test]
fn a_browser_that_connects_or_leaves_publishes_the_setup_part() {
    use crate::language::setup::Stage;

    let bench = SetupBench::start();
    bench.computer.install("chrome");
    bench.facade.refresh_live_parts();
    assert_eq!(
        bench.setup_hints(),
        [bench.setup_etag()],
        "the first look announces the part"
    );
    assert_eq!(bench.facade.setup_view().stage, Stage::Browser);
    let reads = bench.computer.reads();

    bench.facade.refresh_live_parts();
    assert_eq!(bench.setup_hints().len(), 1, "nothing changed");
    assert_eq!(
        bench.computer.reads(),
        reads,
        "a look that saw the same browsers and clients builds nothing"
    );

    bench.browser.connect(true);
    bench.facade.refresh_live_parts();
    let hints = bench.setup_hints();
    assert_eq!(hints.len(), 2);
    assert_eq!(hints[1], bench.setup_etag());
    assert_eq!(bench.facade.setup_view().stage, Stage::Agent);

    bench.browser.connect(false);
    bench.facade.refresh_live_parts();
    assert_eq!(bench.setup_hints().len(), 3);
    assert_eq!(bench.facade.setup_view().stage, Stage::Browser);
}

#[test]
fn a_client_whose_session_arrives_publishes_the_setup_part() {
    let bench = SetupBench::start();
    bench.facade.refresh_live_parts();
    assert_eq!(bench.setup_hints().len(), 1);
    let first_build = bench.computer.reads();
    bench.facade.refresh_live_parts();
    assert_eq!(
        bench.computer.reads(),
        first_build,
        "the same look builds nothing"
    );

    bench.workspaces.admit(
        "a client".into(),
        crate::intake::session::IntakeChannel::Mcp,
    );
    bench.facade.refresh_live_parts();
    let arrived = bench.computer.reads();
    assert!(
        arrived > first_build,
        "the arrival of a client builds the part again"
    );
    bench.facade.refresh_live_parts();
    assert_eq!(bench.computer.reads(), arrived);
    assert_eq!(
        bench.setup_hints().len(),
        1,
        "an unknown client changes nothing the view says"
    );
}

#[test]
fn a_rescan_of_the_clients_republishes_the_setup_part() {
    let bench = SetupBench::start();
    bench.facade.refresh_live_parts();
    assert_eq!(bench.setup_hints().len(), 1);

    bench.facade.refresh_harnesses().unwrap();
    assert_eq!(
        bench.setup_hints().len(),
        1,
        "the rescan built the part again and it said the same"
    );

    bench.computer.install("chrome");
    bench.facade.refresh_harnesses().unwrap();
    let hints = bench.setup_hints();
    assert_eq!(
        hints.len(),
        2,
        "the rescan found a browser to add Ghostlight to"
    );
    assert_eq!(hints[1], bench.setup_etag());
}

#[test]
fn completing_setup_publishes_the_finished_part_once() {
    use crate::language::setup::Stage;

    let bench = SetupBench::start();
    bench.browser.connect(true);
    let workspace = bench.workspaces.admit(
        "a client".into(),
        crate::intake::session::IntakeChannel::Mcp,
    );
    bench.facade.refresh_live_parts();
    let before = bench.setup_hints().len();
    assert_eq!(bench.facade.setup_view().stage, Stage::Agent);

    let record = a_request_that_reached_a_browser(workspace.as_str());
    bench
        .projection
        .record(&record, crate::language::audit_health::Storage::Saved);
    let hints = bench.setup_hints();
    assert_eq!(
        hints.len(),
        before + 1,
        "completion announces the part once"
    );
    assert_eq!(hints.last(), Some(&bench.setup_etag()));
    assert_eq!(bench.facade.setup_view().stage, Stage::Done);

    let mut again = a_request_that_reached_a_browser(workspace.as_str());
    again.invocation = "invocation_2".into();
    bench
        .projection
        .record(&again, crate::language::audit_health::Storage::Saved);
    assert_eq!(bench.setup_hints().len(), before + 1);
}

#[test]
fn connecting_a_client_republishes_setup_with_the_row_waiting_for_a_restart() {
    use crate::install::HarnessAction;
    use crate::language::setup::{RowTone, SetupAction, Stage};

    let bench = SetupBench::with_a_client_to_connect();
    bench.browser.connect(true);
    bench.facade.refresh_harnesses().unwrap();
    let view = bench.facade.setup_view();
    assert_eq!(view.stage, Stage::Agent);
    let row = super::setup::test_support::row(&view, "cursor");
    assert!(matches!(
        row.action.as_ref().map(|action| &action.action),
        Some(SetupAction::ConnectAgent { agent }) if agent == "cursor"
    ));
    let hints = bench.setup_hints().len();

    bench
        .facade
        .manage_harness_expecting("cursor", HarnessAction::Install, "available")
        .unwrap();
    let hinted = bench.setup_hints();
    assert_eq!(hinted.len(), hints + 1, "the install announced the part");
    assert_eq!(hinted.last(), Some(&bench.setup_etag()));
    let view = bench.facade.setup_view();
    let row = super::setup::test_support::row(&view, "cursor");
    assert_eq!(row.detail, "Restart Cursor to finish.");
    assert_eq!(row.tone, RowTone::Attention);
}

#[test]
fn a_session_puts_a_client_in_use_only_once_its_entry_is_in_place_and_the_connections_say_so() {
    use crate::install::HarnessAction;
    use crate::intake::session::IntakeChannel;
    use crate::language::setup::Stage;

    let bench = SetupBench::with_a_client_to_connect();
    bench.browser.connect(true);
    bench.facade.refresh_harnesses().unwrap();
    bench.workspaces.admit("Cursor".into(), IntakeChannel::Mcp);
    bench.facade.refresh_live_parts();
    let published_in_use = || {
        heard(&bench.events)
            .into_iter()
            .rev()
            .filter_map(|event| match event.change {
                WorkbenchChange::ConnectionsChanged { connections } => {
                    Some(connections.harnesses_in_use)
                }
                _ => None,
            })
            .next()
    };
    let snapshot_in_use =
        || super::parts::ConnectionsPart::of(&bench.facade.snapshot()).harnesses_in_use;
    assert_eq!(published_in_use(), Some(Vec::new()));
    assert!(snapshot_in_use().is_empty());
    let view = bench.facade.setup_view();
    assert_eq!(view.stage, Stage::Agent);
    assert_eq!(
        super::setup::test_support::row(&view, "cursor").detail,
        "Found"
    );

    bench
        .facade
        .manage_harness_expecting("cursor", HarnessAction::Install, "available")
        .unwrap();
    assert_eq!(published_in_use(), Some(vec!["cursor".to_owned()]));
    assert_eq!(snapshot_in_use(), ["cursor"]);
    let view = bench.facade.setup_view();
    assert_eq!(
        super::setup::test_support::row(&view, "cursor").detail,
        "Connected"
    );
}

/// A policy file that sets the browser startup posture to `posture`.
fn startup_policy(posture: &str) -> Scratch {
    let path = Scratch::file("workbench-startup.json");
    std::fs::write(
        &path,
        format!(
            r#"{{"schema":4,"name":"test","version":"1","grants":[],"config":[{{"key":"browser.startup","value":"{posture}","level":"mandatory"}}]}}"#
        ),
    )
    .unwrap();
    path
}

#[test]
fn with_no_browser_the_glance_says_which_browser_a_request_starts_and_only_then() {
    use crate::browser::recovery::RecoveryCandidate;
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::NativeHostState;
    use crate::language::workbench as words;

    let chrome = || RecoveryCandidate {
        id: "chrome".into(),
        name: "Google Chrome".into(),
        package: BrowserPackage::Native,
        package_detail: String::new(),
        registration: NativeHostState::Current,
        ordinary_executable: Some("/installed/chrome".into()),
    };
    for (posture, said) in [
        ("on_demand", "Starts Google Chrome on request"),
        ("manual", words::NO_BROWSER),
    ] {
        let path = startup_policy(posture);
        let facade = facade_over(
            GovernanceFacade::new(Some(path.to_path_buf())),
            ControlledBrowser::new(false, false),
        )
        .with_installed_browsers(vec![chrome()]);
        assert_eq!(
            facade.snapshot().no_browser.as_deref(),
            Some(said),
            "{posture}"
        );
    }
}

#[test]
fn a_browser_that_starts_on_request_leaves_the_band_ready_and_status_passing() {
    use crate::browser::recovery::RecoveryCandidate;
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::NativeHostState;
    use crate::language::readiness::Readiness;
    use crate::language::workbench as words;

    let chrome = RecoveryCandidate {
        id: "chrome".into(),
        name: "Google Chrome".into(),
        package: BrowserPackage::Native,
        package_detail: String::new(),
        registration: NativeHostState::Current,
        ordinary_executable: Some("/installed/chrome".into()),
    };
    for (posture, state, detail, severity, browser_line) in [
        (
            "on_demand",
            Readiness::Ready,
            "Starts Google Chrome when an agent asks.",
            super::DiagnosticSeverity::Passing,
            "Starts Google Chrome when an agent asks.",
        ),
        (
            "manual",
            Readiness::NotConnected,
            Readiness::NotConnected.detail(),
            super::DiagnosticSeverity::Warning,
            words::STATUS_BROWSER_WAITING,
        ),
    ] {
        let path = startup_policy(posture);
        let facade = facade_over(
            GovernanceFacade::new(Some(path.to_path_buf())),
            ControlledBrowser::new(false, false),
        )
        .with_installed_browsers(vec![chrome.clone()]);
        let snapshot = facade.snapshot();
        assert_eq!(snapshot.readiness.state, state, "{posture}");
        assert_eq!(snapshot.readiness.detail, detail, "{posture}");
        let card = snapshot
            .diagnostics
            .iter()
            .find(|item| item.id == "browser")
            .expect("a browser check");
        assert_eq!(card.label, words::STATUS_BROWSER_LABEL);
        assert_eq!(
            (card.severity, card.detail.as_str()),
            (severity, browser_line),
            "{posture}"
        );
    }
}

#[test]
fn the_status_checks_say_ghostlight_browser_rules_and_history_in_plain_words() {
    use crate::language::workbench as words;

    let snapshot = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
    )
    .snapshot();
    let checks: Vec<(&str, &str)> = snapshot
        .diagnostics
        .iter()
        .map(|item| (item.label.as_str(), item.detail.as_str()))
        .collect();
    assert_eq!(checks[0], ("Ghostlight", words::STATUS_SERVICE_RUNNING));
    assert_eq!(checks[1].0, "Browser");
    assert!(
        checks[1]
            .1
            .starts_with("Ghostlight in Browser is connected in "),
        "{}",
        checks[1].1
    );
    assert_eq!(checks[2], ("Rules", "No rules apply."));
    assert_eq!(checks[3], ("History", "History is saving."));
}

#[test]
fn with_no_browser_and_no_installed_one_the_glance_says_no_browser() {
    use crate::browser::recovery::RecoveryCandidate;
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::NativeHostState;
    use crate::language::workbench as words;

    let chrome = || RecoveryCandidate {
        id: "chrome".into(),
        name: "Google Chrome".into(),
        package: BrowserPackage::Native,
        package_detail: String::new(),
        registration: NativeHostState::Current,
        ordinary_executable: Some("/installed/chrome".into()),
    };

    let path = startup_policy("on_demand");
    let facade = facade_over(
        GovernanceFacade::new(Some(path.to_path_buf())),
        ControlledBrowser::new(false, false),
    )
    .with_installed_browsers(Vec::new());
    assert_eq!(
        facade.snapshot().no_browser.as_deref(),
        Some(words::NO_BROWSER),
        "no installed browser starts"
    );
    drop(path);

    let connected = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
    )
    .with_installed_browsers(vec![chrome()]);
    assert_eq!(connected.snapshot().no_browser, None);
}

#[test]
fn a_snapshot_never_waits_on_another_reading_the_installed_browsers() {
    use crate::browser::recovery::RecoveryCandidate;
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::NativeHostState;
    use crate::language::workbench as words;
    use std::sync::mpsc;

    let path = startup_policy("on_demand");
    let facade = facade_over(
        GovernanceFacade::new(Some(path.to_path_buf())),
        ControlledBrowser::new(false, false),
    );
    let (entered, reading) = mpsc::channel();
    let (finish, held) = mpsc::channel::<()>();
    let held = Mutex::new(held);
    facade.live.use_browser_inspection(move || {
        entered.send(()).unwrap();
        held.lock().unwrap().recv().unwrap();
        Vec::new()
    });
    std::thread::scope(|scope| {
        let slow = scope.spawn(|| facade.snapshot().no_browser);
        reading.recv().unwrap();
        // The read is held open; the installed browsers change without waiting for it.
        facade.live.use_installed_browsers(vec![RecoveryCandidate {
            id: "chrome".into(),
            name: "Google Chrome".into(),
            package: BrowserPackage::Native,
            package_detail: String::new(),
            registration: NativeHostState::Current,
            ordinary_executable: Some("/installed/chrome".into()),
        }]);
        finish.send(()).unwrap();
        assert_eq!(slow.join().unwrap().as_deref(), Some(words::NO_BROWSER));
    });
    // The held read began before the change, so its answer was not kept.
    assert_eq!(
        facade.snapshot().no_browser.as_deref(),
        Some("Starts Google Chrome on request")
    );
}

#[test]
fn each_browser_counts_the_tabs_of_the_sessions_that_work_in_it() {
    use crate::browser::adapter::{BrowserReadiness, PhysicalTab};
    use crate::intake::session::IntakeChannel;

    let workspaces = crate::workspace::WorkspaceStore::default();
    for (tab_id, label) in [(1, "first"), (2, "second")] {
        let workspace = workspaces.admit(label.into(), IntakeChannel::Mcp);
        let lease = workspaces.acquire(&workspace).unwrap();
        lease
            .add_tab(
                "browser-1",
                &PhysicalTab {
                    tab_id,
                    url: "https://example.com/".into(),
                    title: "test".into(),
                    active: true,
                    readiness: BrowserReadiness::Complete,
                },
            )
            .unwrap();
    }
    workspaces.admit("elsewhere".into(), IntakeChannel::Mcp);
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let snapshot = facade.snapshot();
    assert_eq!(snapshot.browsers.len(), 1);
    assert_eq!(snapshot.browsers[0].tab_count, 2);
}

/// A recorded navigation by `invocation` that landed on `host`, settled at `timestamp_ms`.
fn a_navigation_to(invocation: &str, host: &str, timestamp_ms: u64) -> AuditRecord {
    let mut record = a_request_that_reached_a_browser("workspace_1");
    record.invocation = invocation.into();
    record.tool = "browser_navigate".into();
    record.summary = format!("Opened {host}.");
    record.observed.host = Some(host.into());
    record.timestamp_ms = timestamp_ms;
    record
}

/// A facade whose history holds `records`, oldest first.
fn a_history_of(records: &[AuditRecord]) -> WorkbenchFacade {
    let (facade, projection) = facade_with(&crate::workspace::WorkspaceStore::default());
    for record in records {
        projection.record(record, crate::language::audit_health::Storage::Saved);
    }
    facade
}

#[test]
fn a_recorded_action_is_found_by_its_host_and_named_by_its_tool_and_its_sentence() {
    let facade = a_history_of(&[
        a_navigation_to("invocation_1", "example.com", 1_000),
        a_navigation_to("invocation_2", "sylin.org", 2_000),
    ]);
    let hits = facade.search("SYLIN");
    assert_eq!(hits.len(), 1, "{hits:?}");
    let hit = &hits[0];
    assert_eq!(hit.kind, super::SearchKind::History);
    assert_eq!(hit.id, "invocation_2");
    assert_eq!(hit.title, "navigate: Opened sylin.org.");
    assert_eq!(hit.timestamp_ms, Some(2_000));
    assert_eq!(hit.outcome, None);
    assert!(hit.detail.ends_with("Google Chrome"), "{}", hit.detail);
    assert!(!hit.detail.contains("workspace_"), "{}", hit.detail);
}

#[test]
fn recorded_actions_are_found_by_their_agent_and_their_tool_newest_first() {
    let facade = a_history_of(&[
        a_navigation_to("invocation_1", "example.com", 1_000),
        a_navigation_to("invocation_2", "sylin.org", 2_000),
    ]);
    let agent = crate::language::provenance::EARLIER_SESSION.to_ascii_lowercase();
    for query in [
        agent.as_str(),
        "navigate",
        "browser_navigate",
        "google chrome",
    ] {
        let found: Vec<String> = facade
            .search(query)
            .into_iter()
            .filter(|hit| hit.kind == super::SearchKind::History)
            .map(|hit| hit.id)
            .collect();
        assert_eq!(found, ["invocation_2", "invocation_1"], "{query}");
    }
}

#[test]
fn a_search_never_matches_on_an_opaque_id() {
    let facade = a_history_of(&[a_navigation_to("invocation_1", "example.com", 1_000)]);
    for query in ["invocation_1", "workspace_1", "authority_1"] {
        assert!(facade.search(query).is_empty(), "{query}");
    }
}

#[test]
fn a_recorded_action_that_did_not_succeed_carries_its_outcome_and_is_found_by_it() {
    use crate::language::audit::AuditRefusal;
    use crate::language::history::OutcomeClass;

    let mut failed = a_navigation_to("invocation_1", "example.com", 1_000);
    failed.status = "failed".into();
    let mut kept = a_navigation_to("invocation_2", "example.com", 2_000);
    kept.status = "blocked".into();
    kept.refusal_facts = Some(AuditRefusal::LocalInterlock);
    let mut blocked = a_navigation_to("invocation_3", "example.com", 3_000);
    blocked.status = "blocked".into();
    blocked.allowed = false;
    blocked.reason = crate::governance::ReasonCode::HostDenied;
    let mut credential = a_navigation_to("invocation_4", "example.com", 4_000);
    credential.status = "blocked".into();
    credential.refusal_facts = Some(AuditRefusal::CredentialAuthorization);
    let facade = a_history_of(&[failed, kept, blocked, credential]);
    let outcomes: Vec<_> = facade
        .search("example.com")
        .into_iter()
        .map(|hit| (hit.id, hit.outcome))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("invocation_4".to_owned(), Some(OutcomeClass::NeedsYou)),
            ("invocation_3".to_owned(), Some(OutcomeClass::Refused)),
            ("invocation_2".to_owned(), Some(OutcomeClass::Kept)),
            ("invocation_1".to_owned(), Some(OutcomeClass::Failed)),
        ]
    );
    let found = |query: &str| -> Vec<String> {
        facade.search(query).into_iter().map(|hit| hit.id).collect()
    };
    // Each is found by the word its row wears.
    assert_eq!(found("blocked"), ["invocation_3"]);
    assert_eq!(found("needs you"), ["invocation_4"]);
    assert_eq!(found("kept"), ["invocation_2"]);
    let wire = serde_json::to_value(&facade.search("example.com")[1]).unwrap();
    assert_eq!(wire["outcome"], "refused");
}

#[test]
fn a_credential_refusal_row_says_needs_you() {
    use crate::language::audit::AuditRefusal;
    use crate::language::history::OutcomeClass;

    let mut credential = a_navigation_to("invocation_1", "example.com", 1_000);
    credential.status = "blocked".into();
    credential.refusal_facts = Some(AuditRefusal::CredentialAuthorization);
    let row = &a_history_of(&[credential]).snapshot().history[0];
    assert_eq!(row.outcome, OutcomeClass::NeedsYou);
    assert_eq!(serde_json::to_value(row).unwrap()["outcome"], "needs_you");
}

#[test]
fn a_kept_tab_row_says_kept() {
    use crate::language::audit::AuditRefusal;
    use crate::language::history::OutcomeClass;

    let mut kept = a_navigation_to("invocation_1", "example.com", 1_000);
    kept.tool = "browser_tabs".into();
    kept.status = "blocked".into();
    kept.refusal_facts = Some(AuditRefusal::LocalInterlock);
    // A record blocked with no refusal of its own was refused, never kept: the class says which.
    let mut refused = a_navigation_to("invocation_2", "example.com", 2_000);
    refused.status = "blocked".into();
    let snapshot = a_history_of(&[kept, refused]).snapshot();
    let outcome = |invocation: &str| {
        snapshot
            .history
            .iter()
            .find(|row| row.invocation == invocation)
            .map(|row| row.outcome)
    };
    assert_eq!(outcome("invocation_1"), Some(OutcomeClass::Kept));
    assert_eq!(outcome("invocation_2"), Some(OutcomeClass::Refused));
}

#[test]
fn a_session_hit_says_its_card_line_and_an_operation_names_no_workspace() {
    use crate::intake::session::IntakeChannel;

    let workspaces = crate::workspace::WorkspaceStore::default();
    workspaces.admit("Some Agent".into(), IntakeChannel::Mcp);
    let (facade, projection) = facade_with(&workspaces);
    let session = facade.snapshot().sessions[0].clone();
    projection.react(&DomainEvent::WorkStarted {
        provenance: crate::provenance::ConnectionEvidence::claimed(
            "Some Agent",
            IntakeChannel::Mcp,
        )
        .details(),
        invocation: "invocation_1".into(),
        workspace: session.id.clone(),
        tool: "browser_fill_form".into(),
        activity: PresentationActivity::Fill,
        capabilities: Capability::Write.into(),
        received_at_ms: crate::audit::unix_ms(),
    });
    let hits = facade.search("some agent");
    let line = hits
        .iter()
        .find(|hit| hit.kind == super::SearchKind::Session)
        .expect("the session is found by its agent");
    assert_eq!(line.detail, "No tabs, working");
    let operation = facade
        .search("fill form")
        .into_iter()
        .find(|hit| hit.kind == super::SearchKind::Operation)
        .expect("the work in flight is found by its tool in words");
    assert_eq!(operation.title, "fill form");
    assert!(
        !operation.detail.contains("workspace_"),
        "{}",
        operation.detail
    );
    assert!(
        operation.detail.ends_with("running"),
        "{}",
        operation.detail
    );
}

#[test]
fn the_search_offers_the_browser_work_section_for_browser_work() {
    use crate::language::workbench as words;

    let facade = facade_over(
        GovernanceFacade::new(None),
        ControlledBrowser::new(false, false),
    );
    let hit = facade
        .search("browser work")
        .into_iter()
        .find(|hit| hit.kind == super::SearchKind::Configuration)
        .expect("the browser-work setting is found by its label");
    assert_eq!(hit.title, words::CONTROLS_RUNTIME_LABEL);
    assert_eq!(hit.detail, words::CONTROLS_RUNTIME_ACTIVE);
    assert_eq!(hit.view, super::SearchDestination::BrowserWork);
    // The window opens the section that shows the setting, not Settings as it was left.
    let word = serde_json::to_value(hit.view).expect("a destination is a word");
    assert_eq!(word, "browser_work");
    assert!(
        include_str!("../../ui/lib/words.js").contains(r#"browser_work: "settings/browser-work""#)
    );
}

#[test]
fn the_window_keeps_as_many_recorded_actions_as_the_service_holds() {
    assert!(include_str!("../../ui/lib/words.js")
        .contains(&format!("const HISTORY_HELD = {};", super::HISTORY_LIMIT)));
}
