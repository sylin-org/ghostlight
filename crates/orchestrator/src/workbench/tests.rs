//! Tests of the workbench read model and its facade, `mod.rs`: how domain events and audit
//! records become live work, history and numbered changes, and what the facade answers the
//! window, the tray and every Settings page (readiness, search, previews, runtime control, the
//! opener, the setup part and the one agent count). Each test names the ACCEPTANCE gate, contract
//! or defect it protects. The loopback server's own tests are in `web/tests.rs`, which borrows
//! [`RecordingActs`] from here.

use std::io;
use std::sync::{Arc, Mutex};

use crate::audit::{AuditRecord, AuditRecorder, AuditSink};
use crate::browser::adapter::PresentationActivity;
use crate::events::{DenialPresentation, DomainEvent};
use crate::governance::{Capability, CapabilitySet, Decision, GovernanceFacade};
use crate::intake::session::IntakeChannel;
use crate::language::outcome::Observed;
use crate::provenance::ConnectionEvidence;
use crate::scratch::Scratch;

use super::{
    NotificationKind, PageAddress, WorkbenchChange, WorkbenchEvent, WorkbenchEventSink,
    WorkbenchFacade, WorkbenchPresentationError, WorkbenchPresentationPort, WorkbenchProjection,
};

/// The time the work here was received at, fixed so two runs of one sequence compare equal.
const RECEIVED_AT_MS: u64 = 1;

/// The browser the recorded work here ran in, as a browser reports its name.
const CHROME: &str = "Google Chrome";

/// The id of the Status check that reads the rules in force.
const AUTHORITY_CHECK: &str = "authority";

/// The id of the Status check that reads the browser.
const BROWSER_CHECK: &str = "browser";

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

/// A presentation port whose every native act fails.
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

/// The connection of a client on MCP that calls itself `name`.
fn client(name: &str) -> ConnectionEvidence {
    ConnectionEvidence::claimed(name, IntakeChannel::Mcp)
}

/// Work `invocation` of `workspace`, sent over `from`, that starts running `tool`.
fn started(
    from: &ConnectionEvidence,
    invocation: &str,
    workspace: &str,
    tool: &str,
    activity: PresentationActivity,
    capability: Capability,
) -> DomainEvent {
    DomainEvent::WorkStarted {
        provenance: from.details(),
        invocation: invocation.into(),
        workspace: workspace.into(),
        tool: tool.into(),
        activity,
        capabilities: capability.into(),
        received_at_ms: RECEIVED_AT_MS,
    }
}

/// A `browser_read` of `workspace`, sent over `from`, that starts running.
fn reading(from: &ConnectionEvidence, invocation: &str, workspace: &str) -> DomainEvent {
    started(
        from,
        invocation,
        workspace,
        "browser_read",
        PresentationActivity::Read,
        Capability::Read,
    )
}

/// A `browser_read` of `workspace`, sent over `from`, that waits for earlier browser work.
fn waiting(from: &ConnectionEvidence, invocation: &str, workspace: &str) -> DomainEvent {
    DomainEvent::WorkWaiting {
        provenance: from.details(),
        invocation: invocation.into(),
        workspace: workspace.into(),
        tool: "browser_read".into(),
        activity: PresentationActivity::Read,
        capabilities: Capability::Read.into(),
        received_at_ms: RECEIVED_AT_MS,
    }
}

/// The audit record of a `browser_read` that succeeded on `host`.
fn a_read_on(invocation: &str, workspace: &str, host: &str) -> AuditRecord {
    AuditRecord::now(
        invocation,
        workspace,
        "browser_read",
        Capability::Read,
        GovernanceFacade::new(None).snapshot().id(),
        Decision::permitted(),
        "succeeded",
        "none",
        &crate::language::outcome::Outcome::TextRead {
            words: 12,
            host: Some(host.into()),
        }
        .audit(),
        5,
    )
    .with_observation(Observed {
        host: Some(host.into()),
        ..Observed::default()
    })
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

/// The Status check `id` of `snapshot`.
fn check(snapshot: &super::WorkbenchSnapshot, id: &str) -> super::DiagnosticItem {
    snapshot
        .diagnostics
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("the Status page always carries the {id} check"))
}

fn temporary_policy(name: &str) -> Scratch {
    Scratch::file(&format!("workbench-{name}.json"))
}

const READ_ONLY_POLICY: &str = r#"{"schema":4,"name":"read","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[]}"#;

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

fn statuses(events: &[WorkbenchEvent]) -> Vec<super::parts::StatusPart> {
    events
        .iter()
        .filter_map(|event| match &event.change {
            WorkbenchChange::StatusChanged { status } => Some(status.clone()),
            _ => None,
        })
        .collect()
}

/// Every script the window loads, in the order the page loads it, as one text.
fn surface_source() -> String {
    [
        include_str!("../../ui/lib/words.js"),
        include_str!("../../ui/lib/entries.js"),
        include_str!("../../ui/lib/transport.js"),
        include_str!("../../ui/lib/icons.js"),
        include_str!("../../ui/lib/setup-view.js"),
        include_str!("../../ui/lib/store.js"),
        include_str!("../../ui/lib/view.js"),
        include_str!("../../ui/kit/settings.js"),
        include_str!("../../ui/app.js"),
    ]
    .join("\n")
}

/// Protects desktop gate 2 and the executor's "the workbench shows waiting becoming running
/// once" (ACCEPTANCE): work shows as waiting, held or running, only running work makes a session
/// and the band read Working (7cf3b39e, 7312a096), and its audit record moves it from the live
/// set into history with what it observed.
#[test]
fn work_in_flight_is_shown_as_it_waits_runs_and_settles() {
    use super::OperationPhase;
    use crate::language::readiness::Readiness;

    let evidence = Arc::new(client("test"));
    let workspaces = crate::workspace::WorkspaceStore::default();
    let workspace = workspaces.connect(Arc::clone(&evidence), None);
    let workspace = workspace.as_str();
    let projection = WorkbenchProjection::default();
    let notifications = Arc::new(Notifications::default());
    projection.attach_presentation(notifications.clone());
    let facade = WorkbenchFacade::new(
        projection.clone(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );

    projection.react(&waiting(&evidence, "invocation_waiting", workspace));
    let operations = projection.operations();
    assert_eq!(operations[0].phase, OperationPhase::Waiting);
    assert_eq!(
        operations[0].activity,
        crate::language::outcome::WAITING_FOR_WORK
    );
    assert!(notifications.0.lock().unwrap().is_empty());

    projection.react(&reading(&evidence, "invocation_held", workspace));
    projection.react(&DomainEvent::HoldEntered {
        invocation: "invocation_held".into(),
        workspace: workspace.into(),
        physical_id: 1,
    });
    let shown = facade.snapshot();
    let phase = |invocation: &str| {
        shown
            .operations
            .iter()
            .find(|operation| operation.invocation == invocation)
            .map(|operation| operation.phase)
    };
    assert_eq!(phase("invocation_waiting"), Some(OperationPhase::Waiting));
    assert_eq!(phase("invocation_held"), Some(OperationPhase::Held));
    assert_eq!(
        shown.sessions[0].running_operations, 0,
        "held and waiting work is not running"
    );
    assert_eq!(shown.readiness.state, Readiness::Ready);

    projection.react(&reading(&evidence, "invocation_running", workspace));
    let shown = facade.snapshot();
    assert_eq!(shown.sessions[0].running_operations, 1);
    assert_eq!(shown.readiness.state, Readiness::Working);

    let durable = Arc::new(MemoryAudit::default());
    let recorder = AuditRecorder::new(durable.clone(), projection.clone());
    assert_eq!(
        recorder.record(&a_read_on("invocation_running", workspace, "example.com")),
        crate::language::audit_health::Storage::Saved
    );
    assert!(projection
        .operations()
        .iter()
        .all(|operation| operation.invocation != "invocation_running"));
    let history = projection.history();
    assert_eq!(history[0].invocation, "invocation_running");
    assert_eq!(history[0].tool, "browser_read");
    assert_eq!(
        history[0].observed.host.as_deref(),
        Some("example.com"),
        "history dropped what the action was observed doing"
    );
    assert_eq!(durable.0.lock().unwrap().len(), 1);
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

/// Guards the contract the window's store resyncs by (`WorkbenchEvent.seq`; ui/lib/store.js and
/// tests/workbench-store.test.mjs): every change is numbered once, in order, whether or not anyone
/// listens; every sink hears the same numbers; and a snapshot never claims a change that landed
/// while it was built, which the page would then skip.
#[test]
fn the_snapshot_and_the_stream_share_one_gapless_sequence() {
    let evidence = client("test");
    let projection = WorkbenchProjection::default();
    projection.react(&reading(&evidence, "invocation_unheard", "workspace_1"));
    assert_eq!(
        projection.current_seq(),
        1,
        "a change nobody listened to still has a number"
    );

    let first = Arc::new(Events::default());
    let second = Arc::new(Events::default());
    projection.attach_events(first.clone());
    projection.attach_events(second.clone());
    projection.react(&reading(&evidence, "invocation_1", "workspace_1"));
    projection.react(&DomainEvent::WorkCompleted {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        physical_id: None,
    });
    AuditRecorder::new(Arc::new(MemoryAudit::default()), projection.clone()).record(&a_read_on(
        "invocation_1",
        "workspace_1",
        "example.com",
    ));

    let one = heard(&first);
    assert_eq!(
        one.iter().map(|event| event.seq).collect::<Vec<_>>(),
        [2, 3, 4],
        "a sink attached later hears only what follows, numbered where the sequence stood"
    );
    assert!(matches!(
        one[0].change,
        WorkbenchChange::OperationStarted { .. }
    ));
    assert!(matches!(
        one[1].change,
        WorkbenchChange::OperationChanged { .. }
    ));
    assert!(matches!(
        one[2].change,
        WorkbenchChange::OperationSettled { .. }
    ));
    assert_eq!(one, heard(&second));
    assert_eq!(projection.current_seq(), 4);

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

/// Protects desktop gate 6: blocked work asks for at most one native notice per invocation, the
/// notice names no invocation and no workspace, and a notice that fails changes nothing the
/// projection holds.
#[test]
fn a_blocked_operation_asks_for_one_content_free_notice_and_a_failed_notice_changes_nothing() {
    let blocked = DomainEvent::WorkBlocked {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        physical_id: Some(7),
        presentation: DenialPresentation::Guardrail,
    };
    let evidence = client("test");
    let run = |port: Arc<dyn WorkbenchPresentationPort>| {
        let projection = WorkbenchProjection::default();
        projection.attach_presentation(port);
        projection.react(&reading(&evidence, "invocation_1", "workspace_1"));
        projection.react(&blocked);
        projection.react(&blocked);
        projection.operations()
    };

    let notifications = Arc::new(Notifications::default());
    let told = run(notifications.clone());
    let notices = notifications.0.lock().unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].kind, NotificationKind::Blocked);
    let encoded = serde_json::to_string(&notices[0]).unwrap();
    for named in ["invocation_1", "workspace_1"] {
        assert!(!encoded.contains(named), "the notice names {named}");
    }

    let failed = run(Arc::new(Failing));
    assert_eq!(
        failed, told,
        "a failed notice changed what the projection holds"
    );
    assert_eq!(failed[0].phase, super::OperationPhase::Blocked);
}

/// Protects ADR-0122 Decision 7 and Grouped history 4 ("policy previews count actual recorded
/// child operations"), and reproduces e2752439 (ADR-0203 D7.3, G-06): a draft is checked against
/// recorded work the way `ghostlight policy simulate` replays it, on each action's own channel (a
/// record that kept none arrived on MCP), its host and its capabilities. A draft with no rules
/// explains itself instead of reporting a count.
#[test]
fn a_preview_replays_recorded_work_through_the_draft_as_simulate_does() {
    /// A facade whose history holds `records`, each a recorded action with its tool, host,
    /// capability and channel.
    fn recorded(
        records: &[(
            &'static str,
            &'static str,
            &'static str,
            Capability,
            Option<IntakeChannel>,
        )],
    ) -> WorkbenchFacade {
        let projection = WorkbenchProjection::default();
        let audit = AuditRecorder::new(Arc::new(MemoryAudit::default()), projection.clone());
        for (invocation, tool, host, capability, channel) in records {
            let mut record = AuditRecord::now(
                invocation,
                "workspace_1",
                tool,
                CapabilitySet::one(*capability),
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
                host: Some((*host).into()),
                ..Observed::default()
            });
            record.channel = *channel;
            audit.record(&record);
        }
        WorkbenchFacade::new(
            projection,
            crate::workspace::WorkspaceStore::default(),
            GovernanceFacade::new(None),
            ControlledBrowser::new(false, false),
            crate::diagnostics::DiagnosticsHub::for_tests(),
        )
    }
    let refused = |preview: &super::PolicyPreview, tool: &str| {
        preview
            .refused
            .iter()
            .find(|entry| entry.tool == tool)
            .map_or(0, |entry| entry.count)
    };

    let mcp = Some(IntakeChannel::Mcp);
    let by_host = recorded(&[
        ("i1", "browser_read", "example.com", Capability::Read, mcp),
        ("i2", "browser_read", "example.com", Capability::Read, mcp),
        (
            "i3",
            "browser_execute",
            "example.com",
            Capability::Execute,
            mcp,
        ),
        (
            "i4",
            "browser_read",
            "elsewhere.test",
            Capability::Read,
            mcp,
        ),
    ]);
    let reading_here = by_host
        .preview_user_policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
    assert_eq!(reading_here.considered, 4);
    // Two reads on example.com survive; the page-code run and the other host do not.
    assert_eq!(reading_here.refused_total, 2);
    assert_eq!(refused(&reading_here, "browser_execute"), 1);
    assert_eq!(refused(&reading_here, "browser_read"), 1);
    let open = by_host
        .preview_user_policy(
            r#"{"schema":4,"name":"open","version":"1","grants":[{"id":"everything","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]}"#,
        )
        .unwrap();
    assert_eq!(open.refused_total, 0);
    // A draft with no rules refuses everything by definition; the sentence explains the draft
    // instead of reporting a number that reads as a claim about work already done.
    let empty = by_host
        .preview_user_policy(r#"{"schema":4,"name":"mine","version":"1","grants":[]}"#)
        .unwrap();
    assert_eq!(empty.refused_total, 4);
    assert!(empty.refused.is_empty());
    assert!(
        !empty
            .summary
            .chars()
            .any(|character| character.is_ascii_digit()),
        "{}",
        empty.summary
    );
    assert!(by_host.preview_user_policy("not a policy").is_err());

    // Every site and every capability is allowed: only the MCP switch can refuse (e2752439).
    let by_channel = recorded(&[
        ("i1", "browser_read", "example.com", Capability::Read, mcp),
        ("i2", "browser_click", "example.com", Capability::Read, mcp),
        (
            "i3",
            "browser_read",
            "example.com",
            Capability::Read,
            Some(IntakeChannel::Cli),
        ),
        ("i4", "browser_click", "example.com", Capability::Read, None),
    ]);
    let mcp_off = by_channel
        .preview_user_policy(
            r#"{"schema":4,"name":"no agents","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}],"config":[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"}]}"#,
        )
        .unwrap();
    assert_eq!(mcp_off.considered, 4);
    assert_eq!(mcp_off.refused_total, 3, "{mcp_off:?}");
    assert_eq!(refused(&mcp_off, "browser_click"), 2, "{mcp_off:?}");
    assert_eq!(
        refused(&mcp_off, "browser_read"),
        1,
        "the command line's read stays allowed: {mcp_off:?}"
    );
}

/// Protects desktop gates 19 and 7: Pause, Resume, End session and Start a new session are
/// offered and applied through the authority whether a browser is connected, connected but
/// unreachable, or absent. Each confirmation states only the resulting authority state,
/// identically whether publication to adapters succeeded, failed or reached none, and makes no
/// browser delivery claim. The controls part every Settings page draws offers exactly the acts
/// governance offers for that state, to the window and a watcher alike (A-01, 37554eab, ADR-0203
/// D12.1), and no control dispatches a browser primitive (`ControlledBrowser::call` panics).
#[test]
fn a_runtime_control_is_offered_and_applied_by_the_authority_with_or_without_a_browser() {
    use super::context::WebContext;
    use super::WorkbenchRuntimeIntent;
    use crate::browser::adapter::RuntimeControlState;
    use crate::language::readiness::Readiness;
    use serde_json::json;

    let order = [
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
    ];
    // Connected and published, connected but publication failed, and no browser at all.
    for (connected, publish_fails) in [(true, false), (true, true), (false, false)] {
        let case = format!("connected {connected}, publication fails {publish_fails}");
        let browser = ControlledBrowser::new(connected, publish_fails);
        let governance = GovernanceFacade::new(None);
        governance.relay_runtime_state(browser.clone());
        let facade = facade_over(governance.clone(), browser.clone());
        assert!(facade.snapshot().readiness.invites_control, "{case}");
        for (intent, state) in order {
            let result = facade.apply_runtime_intent(intent);
            assert!(result.accepted, "{case}: {intent:?}");
            assert_eq!(result.runtime_state, state, "{case}");
            assert_eq!(
                result.message,
                crate::language::control::confirmation(state),
                "{case}"
            );
            assert!(
                serde_json::to_value(&result)
                    .unwrap()
                    .get("browser_notified")
                    .is_none(),
                "{case}: the result makes a browser delivery claim"
            );
            assert_eq!(governance.runtime_state(), state, "{case}");
            assert_eq!(
                governance.runtime_decision().allowed,
                state == RuntimeControlState::Active,
                "{case}: the authority enforces {state:?}"
            );
            let snapshot = facade.snapshot();
            assert!(snapshot.readiness.invites_control, "{case}");
            let readiness = match state {
                RuntimeControlState::Held => Readiness::Paused,
                RuntimeControlState::Ended => Readiness::SessionEnded,
                RuntimeControlState::Active if connected => Readiness::Ready,
                RuntimeControlState::Active => Readiness::NotConnected,
            };
            assert_eq!(snapshot.readiness.state, readiness, "{case}: {state:?}");
            let offered = json!(crate::governance::offers(state));
            for (audience, controls) in [
                ("window", snapshot.controls.clone()),
                ("watcher", snapshot.controls.seen_by(WebContext::Watch)),
            ] {
                assert_eq!(
                    json!(controls)["runtime"]["offers"],
                    offered,
                    "{case}: the {audience}'s controls in {state:?}"
                );
            }
        }
        assert_eq!(
            *browser.published.lock().unwrap(),
            order.map(|(_, state)| state),
            "{case}: every adapter still gets the best-effort notice"
        );
    }
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

/// Protects desktop gate 18 and the band's one answer: the band, the Status rules and browser
/// checks and the Policy tab read what governance and browser recovery hold. Invalid rules read
/// Policy not valid and fail closed; a broken edit to rules already in force stays Ready, keeps
/// its Policy situation and shows a Status warning; a switched-off channel is said on the band
/// and in the rules check (842ef606); with no browser connected, the startup posture and the
/// installed browsers decide whether the band reads Ready, what At a glance says in place of a
/// browser, and the Status browser check (7a5065d3 j-07); a connected browser leaves no such line.
#[test]
fn the_band_and_the_status_checks_read_one_answer_for_each_situation() {
    use super::DiagnosticSeverity;
    use crate::browser::recovery::RecoveryCandidate;
    use crate::governance::effective::Situation;
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::NativeHostState;
    use crate::language::channel_switch::ChannelsOff;
    use crate::language::outcome::PolicyAuthor;
    use crate::language::readiness::{starts_when_asked, Readiness};
    use crate::language::workbench as words;

    /// What one situation is made of.
    struct Given {
        rules: Option<String>,
        broken_after: bool,
        installed: Option<Vec<RecoveryCandidate>>,
        connected: bool,
    }
    /// What the band, one Status check, At a glance and the Policy tab say in it.
    struct Said {
        readiness: Readiness,
        detail: String,
        check: &'static str,
        severity: Option<DiagnosticSeverity>,
        check_detail: Option<String>,
        no_browser: Option<Option<String>>,
        situation: Option<Situation>,
    }

    let chrome = || {
        vec![RecoveryCandidate {
            id: "chrome".into(),
            name: CHROME.into(),
            package: BrowserPackage::Native,
            package_detail: String::new(),
            registration: NativeHostState::Current,
            ordinary_executable: Some("/installed/chrome".into()),
            extension: true,
        }]
    };
    let switched = |config: &str| {
        Some(format!(
            r#"{{"schema":4,"name":"mine","version":"1","grants":[{{"id":"all","hosts":{{"allow":["*"]}},"allowed":["read"]}}],"config":{config}}}"#
        ))
    };
    let startup = |posture: &str| Some(std::fs::read_to_string(startup_policy(posture)).unwrap());
    let given =
        |rules: Option<String>, installed: Option<Vec<RecoveryCandidate>>, connected| Given {
            rules,
            broken_after: false,
            installed,
            connected,
        };
    let mcp_off = ChannelsOff {
        mcp: Some(PolicyAuthor::User),
        ..ChannelsOff::default()
    };
    let both_off = ChannelsOff {
        mcp: Some(PolicyAuthor::User),
        cli: Some(PolicyAuthor::User),
        organization: None,
    };
    let ready = Readiness::Ready.detail().to_owned();
    let not_connected = Readiness::NotConnected.detail().to_owned();

    let situations = [
        (
            "invalid rules",
            given(Some("{half-written".into()), None, true),
            Said {
                readiness: Readiness::PolicyInvalid,
                detail: Readiness::PolicyInvalid.detail().to_owned(),
                check: AUTHORITY_CHECK,
                severity: Some(DiagnosticSeverity::Failing),
                check_detail: None,
                no_browser: None,
                situation: Some(Situation::FailingClosed),
            },
        ),
        (
            "valid rules",
            given(Some(READ_ONLY_POLICY.into()), None, true),
            Said {
                readiness: Readiness::Ready,
                detail: ready.clone(),
                check: AUTHORITY_CHECK,
                severity: Some(DiagnosticSeverity::Passing),
                check_detail: None,
                no_browser: None,
                situation: Some(Situation::UserOnly),
            },
        ),
        (
            "a broken edit to rules in force",
            Given {
                broken_after: true,
                ..given(Some(READ_ONLY_POLICY.into()), None, true)
            },
            Said {
                readiness: Readiness::Ready,
                detail: ready.clone(),
                check: AUTHORITY_CHECK,
                severity: Some(DiagnosticSeverity::Warning),
                check_detail: None,
                no_browser: None,
                situation: Some(Situation::UserOnly),
            },
        ),
        (
            "MCP switched off",
            given(
                switched(r#"[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"}]"#),
                None,
                true,
            ),
            Said {
                readiness: Readiness::Ready,
                detail: mcp_off.ready_line().unwrap().to_owned(),
                check: AUTHORITY_CHECK,
                severity: None,
                check_detail: Some(words::status_rules_in_force(true, false, mcp_off)),
                no_browser: None,
                situation: None,
            },
        ),
        (
            "both channels switched off",
            given(
                switched(
                    r#"[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"},{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]"#,
                ),
                None,
                true,
            ),
            Said {
                readiness: Readiness::TurnedOff,
                detail: both_off.said().unwrap(),
                check: AUTHORITY_CHECK,
                severity: None,
                check_detail: Some(words::status_rules_in_force(true, false, both_off)),
                no_browser: None,
                situation: None,
            },
        ),
        (
            "no browser, one starts on request",
            given(startup("on_demand"), Some(chrome()), false),
            Said {
                readiness: Readiness::Ready,
                detail: starts_when_asked(CHROME),
                check: BROWSER_CHECK,
                severity: Some(DiagnosticSeverity::Passing),
                check_detail: Some(starts_when_asked(CHROME)),
                no_browser: Some(Some(words::no_browser(Some(CHROME)))),
                situation: None,
            },
        ),
        (
            "no browser, started by hand",
            given(startup("manual"), Some(chrome()), false),
            Said {
                readiness: Readiness::NotConnected,
                detail: not_connected.clone(),
                check: BROWSER_CHECK,
                severity: Some(DiagnosticSeverity::Warning),
                check_detail: Some(words::STATUS_BROWSER_WAITING.into()),
                no_browser: Some(Some(words::NO_BROWSER.into())),
                situation: None,
            },
        ),
        (
            "no browser and none installed",
            given(startup("on_demand"), Some(Vec::new()), false),
            Said {
                readiness: Readiness::NotConnected,
                detail: not_connected,
                check: BROWSER_CHECK,
                severity: None,
                check_detail: None,
                no_browser: Some(Some(words::NO_BROWSER.into())),
                situation: None,
            },
        ),
        (
            "a connected browser",
            given(None, Some(chrome()), true),
            Said {
                readiness: Readiness::Ready,
                detail: ready,
                check: BROWSER_CHECK,
                severity: None,
                check_detail: None,
                no_browser: Some(None),
                situation: None,
            },
        ),
    ];

    for (label, given, said) in situations {
        let path = temporary_policy("situation");
        let governance = GovernanceFacade::new(given.rules.as_ref().map(|rules| {
            std::fs::write(&path, rules).unwrap();
            path.to_path_buf()
        }));
        let mut facade = facade_over(
            governance.clone(),
            ControlledBrowser::new(given.connected, false),
        );
        if let Some(installed) = given.installed {
            facade = facade.with_installed_browsers(installed);
        }
        if given.broken_after {
            std::fs::write(&path, "{half-written").unwrap();
            governance.refresh_tick();
        }
        let snapshot = facade.snapshot();
        assert_eq!(snapshot.readiness.state, said.readiness, "{label}");
        assert_eq!(snapshot.readiness.word, said.readiness.word(), "{label}");
        assert_eq!(snapshot.readiness.detail, said.detail, "{label}");
        assert!(snapshot.readiness.invites_control, "{label}");
        let card = check(&snapshot, said.check);
        if let Some(severity) = said.severity {
            assert_eq!(card.severity, severity, "{label}");
        }
        if let Some(detail) = said.check_detail {
            assert_eq!(card.detail, detail, "{label}");
        }
        if let Some(no_browser) = said.no_browser {
            assert_eq!(snapshot.no_browser, no_browser, "{label}");
        }
        if let Some(situation) = said.situation {
            assert_eq!(
                snapshot.configuration.policy.situation, situation,
                "{label}: the Policy situation the band reads"
            );
            assert_eq!(
                facade.policy().situation,
                situation,
                "{label}: the Policy tab"
            );
        }
    }
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

/// A temporary home, a facade over it with a client roster of its own, and acts attached. The
/// home goes when its guard drops, which comes first so it outlives the facade.
fn facade_with_acts() -> (Scratch, WorkbenchFacade, Arc<RecordingActs>) {
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
    facade.attach_native_acts(acts.clone());
    (root, facade, acts)
}

/// Protects desktop gate 11 ("a link the page wants opened goes to the system opener through the
/// service"; the WebView has no arbitrary capability): the page names a destination and the
/// service owns the address. Opening a destination asks the opener for exactly that
/// destination's address, every one of which is the product's own published page, with
/// documentation on `main` and never `dev` (826f2ec1), or Google's Chrome page. A roster host's
/// download opens that host's own https page (de405a2a) and an unknown host or one with no page
/// opens nothing. An address that is not https never reaches the computer, and the page's files
/// carry no address of their own.
#[test]
fn the_system_opener_is_given_only_https_addresses_the_product_chose() {
    use super::{NativeActError, WorkbenchDestination};
    use crate::install::HarnessError;

    let (_root, facade, acts) = facade_with_acts();
    for destination in WorkbenchDestination::all() {
        facade.open_destination(destination).unwrap();
    }
    assert_eq!(
        *acts.urls.lock().unwrap(),
        WorkbenchDestination::all().map(WorkbenchDestination::url)
    );
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
        "documentation links name the published branch"
    );
    assert!(WorkbenchDestination::all()
        .into_iter()
        .all(|destination| !destination.url().contains("/dev/")));

    acts.urls.lock().unwrap().clear();
    let harnesses = facade.cached_harnesses();
    for summary in &harnesses {
        if let Some(url) = &summary.download_url {
            assert!(
                url.starts_with(super::SECURE_SCHEME),
                "{}: {url}",
                summary.id
            );
        }
    }
    let page_of = |id: &str| {
        harnesses
            .iter()
            .find(|summary| summary.id == id)
            .and_then(|summary| summary.download_url.clone())
            .unwrap_or_else(|| panic!("{id} has a download page"))
    };
    for host in ["cline-vscode", "cline-cli"] {
        facade.open_harness_download(host).unwrap();
    }
    assert_eq!(
        *acts.urls.lock().unwrap(),
        [page_of("cline-vscode"), page_of("cline-cli")]
    );
    assert_ne!(
        page_of("cline-vscode"),
        page_of("cline-cli"),
        "the extension's page is not the CLI's"
    );
    assert!(matches!(
        facade.open_harness_download("not-a-harness"),
        Err(NativeActError::Harness(HarnessError::UnknownHarness(_)))
    ));
    assert!(matches!(
        facade.open_harness_download("claude-desktop"),
        Err(NativeActError::Harness(HarnessError::NoDownload(_)))
    ));
    let asked = acts.asked();
    for url in [
        "http://example.com/",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "",
        "HTTPS://example.com/",
    ] {
        assert!(
            matches!(
                facade.open_secure_link(&*acts, url),
                Err(NativeActError::LinkNotSecure)
            ),
            "{url}"
        );
    }
    assert_eq!(
        acts.asked(),
        asked,
        "a refused address reached the computer"
    );

    assert!(
        !include_str!("../../ui/index.html").contains("href=\"http"),
        "the markup carries a raw URL"
    );
    assert!(
        !surface_source().contains(super::SECURE_SCHEME),
        "the page's scripts carry a raw URL"
    );
}

/// Reproduces 7a5065d3 (j-08, window-1) and 3f27d340 (A-03), and protects ADR-0199's amendment of
/// 2026-10-18 and ARCHITECTURE ("the band, Settings and About count nothing of their own"):
/// sessions are counted as the agents At a glance groups them into, connections by their intake,
/// and the controls line, the connections part and the tray tooltip carry the one count the
/// service computes. A command-line session whose call has ended still counts as its agent.
#[test]
fn the_window_the_tray_and_status_count_agents_one_way() {
    use crate::intake::session::SessionMarker;
    use crate::language::workbench::{service_line, ConnectionCounts};

    let workspaces = crate::workspace::WorkspaceStore::default();
    workspaces.admit("Claude Code".into(), IntakeChannel::Mcp);
    workspaces.admit("Claude Code".into(), IntakeChannel::Mcp);
    workspaces.admit("Cursor".into(), IntakeChannel::Mcp);
    let call = Arc::new(ConnectionEvidence::claimed(
        "ghostlight call",
        IntakeChannel::Cli,
    ));
    let script = workspaces.connect(
        Arc::clone(&call),
        Some(SessionMarker::Declared {
            key: "script".into(),
        }),
    );
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        workspaces.clone(),
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let shows = |counts: ConnectionCounts| {
        assert_eq!(facade.connection_counts(), counts);
        let snapshot = facade.snapshot();
        assert_eq!(snapshot.controls.service.word, service_line(Some(counts)));
        let grouped: std::collections::BTreeSet<&str> = snapshot
            .sessions
            .iter()
            .map(|session| session.agent.id.as_str())
            .collect();
        assert_eq!(grouped.len(), counts.agents, "At a glance groups them so");
        assert_eq!(super::parts::ConnectionsPart::of(&snapshot).counts, counts);
        assert!(facade
            .tray_model()
            .tooltip
            .ends_with(&service_line(Some(counts))));
    };

    // Two Claude Code sessions are one agent, the way At a glance groups them.
    let connected = ConnectionCounts {
        agents: 3,
        mcp_clients: 3,
        command_line: 1,
        browsers: 1,
    };
    shows(connected);
    // The line ADR-0199's amendment quotes, as rc-1's c3 slice settled it.
    assert_eq!(
        service_line(Some(connected)),
        "Ghostlight is running; 3 agents and 1 browser connected."
    );

    workspaces.disconnect(
        &script,
        call.attribution().connection_id.as_deref().unwrap(),
    );
    shows(ConnectionCounts {
        command_line: 0,
        ..connected
    });
}

/// The agent a session whose client calls itself `reported` carries, after showing that its
/// running work, its history item and each of the Integrations rows `hosts` carry the same one.
fn one_agent_across_the_window(reported: &str, hosts: &[&str]) -> crate::agent::Agent {
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
    projection.react(&reading(
        &evidence,
        "invocation_running",
        workspace.as_str(),
    ));
    let recorder = AuditRecorder::new(Arc::new(MemoryAudit::default()), projection.clone());
    let record = a_read_on("invocation_done", workspace.as_str(), "example.com")
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

/// A facade over `workspaces`, with no browser and nothing else to show.
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

/// Protects ACCEPTANCE Connection reporting (ADR-0161) and its amendment of 2026-10-18: a session
/// is named for its agent as a person knows it. A client that sent no name is the unknown agent
/// and a name's direction marks are dropped (233e7569); with no claim, the observed source alone
/// names it and its work names the same agent; a command-line session keeps its agent once its
/// call has disconnected, and removes only the closed connection; and the session's work, its
/// history and each of its Integrations rows carry that one agent (68c7eb3e, c360f6c0).
#[test]
fn a_session_is_named_for_its_agent_and_carries_that_one_agent_everywhere() {
    use crate::agent::Agent;
    use crate::intake::session::SessionMarker;
    use crate::provenance::{Attribution, Source};

    let workspaces = crate::workspace::WorkspaceStore::default();
    for unnamed in ["", "   ", "\u{200f}"] {
        workspaces.admit(unnamed.into(), IntakeChannel::Mcp);
    }
    workspaces.admit("Some\u{202e}Agent\u{200f}".into(), IntakeChannel::Mcp);
    let mut named: Vec<String> = facade_with(&workspaces)
        .0
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
    projection.react(&reading(
        &evidence,
        "invocation_running",
        workspace.as_str(),
    ));
    let shown = facade.snapshot();
    assert_eq!(shown.sessions[0].agent.id, "claude-desktop");
    assert_eq!(shown.sessions[0].agent.name, "Claude Desktop");
    assert_eq!(shown.sessions[0].agent, shown.operations[0].agent);

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
    let shown = facade_with(&workspaces).0.snapshot();
    assert!(shown.sessions[0].connections.is_empty());
    assert_eq!(shown.sessions[0].agent, Agent::command_line());
    assert_eq!(shown.sessions[0].channel, IntakeChannel::Cli);

    // The catalog's ids: Codex calls itself codex-mcp-client, and Cline has four hosts.
    assert_eq!(
        one_agent_across_the_window("codex-mcp-client", &["codex"]).id,
        "codex"
    );
    assert_eq!(
        one_agent_across_the_window(
            "cline",
            &[
                "cline-cli",
                "cline-vscode",
                "cline-cursor",
                "cline-windsurf"
            ],
        )
        .id,
        "cline"
    );
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
            name: Some(CHROME.into()),
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

/// The record of an MCP request of `workspace` that read a page in Chrome and succeeded.
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
    .from_channel(Some(IntakeChannel::Mcp))
    .with_browser(Some(CHROME.into()));
    record.timestamp_ms = 1234;
    record
}

/// Protects desktop gates 23 and 24 (the setup view follows the computer live and shows Ready):
/// a rescan that finds a browser, a browser that connects or leaves, a client connected from the
/// window and the first request that completes setup each announce the part once, with the tag a
/// fresh read gives; a look that sees the same inputs announces nothing and reads nothing; and a
/// client whose name matches no catalog agent rebuilds the part but announces nothing.
#[test]
fn the_setup_part_is_announced_when_what_it_says_changes_and_only_then() {
    use crate::install::HarnessAction;
    use crate::language::audit_health::Storage;
    use crate::language::setup::Stage;

    let bench = SetupBench::start();
    bench.facade.refresh_live_parts();
    assert_eq!(
        bench.setup_hints(),
        [bench.setup_etag()],
        "the first look announces the part"
    );
    let reads = bench.computer.reads();
    bench.facade.refresh_live_parts();
    assert_eq!(bench.setup_hints().len(), 1, "nothing changed");
    assert_eq!(
        bench.computer.reads(),
        reads,
        "a look that saw the same browsers and clients builds nothing"
    );

    bench.facade.refresh_harnesses().unwrap();
    assert_eq!(
        bench.setup_hints().len(),
        1,
        "a rescan that sees the same says nothing"
    );
    bench.computer.install("chrome");
    bench.facade.refresh_harnesses().unwrap();
    let hints = bench.setup_hints();
    assert_eq!(hints.len(), 2, "the rescan found a browser");
    assert_eq!(hints[1], bench.setup_etag());
    assert_eq!(bench.facade.setup_view().stage, Stage::Browser);

    bench.browser.connect(true);
    bench.facade.refresh_live_parts();
    let hints = bench.setup_hints();
    assert_eq!(hints.len(), 3);
    assert_eq!(hints[2], bench.setup_etag());
    assert_eq!(bench.facade.setup_view().stage, Stage::Agent);
    bench.browser.connect(false);
    bench.facade.refresh_live_parts();
    assert_eq!(bench.setup_hints().len(), 4);
    assert_eq!(bench.facade.setup_view().stage, Stage::Browser);

    let before = bench.computer.reads();
    bench
        .workspaces
        .admit("a client".into(), IntakeChannel::Mcp);
    bench.facade.refresh_live_parts();
    assert!(
        bench.computer.reads() > before,
        "the arrival of a client builds the part again"
    );
    assert_eq!(
        bench.setup_hints().len(),
        4,
        "an unknown client changes nothing the view says"
    );

    let bench = SetupBench::with_a_client_to_connect();
    bench.browser.connect(true);
    bench.facade.refresh_harnesses().unwrap();
    let hints = bench.setup_hints().len();
    bench
        .facade
        .manage_harness_expecting("cursor", HarnessAction::Install, "available")
        .unwrap();
    let hinted = bench.setup_hints();
    assert_eq!(
        hinted.len(),
        hints + 1,
        "connecting the client announced it"
    );
    assert_eq!(hinted.last(), Some(&bench.setup_etag()));

    let workspace = bench
        .workspaces
        .admit("a client".into(), IntakeChannel::Mcp);
    bench.facade.refresh_live_parts();
    let before = bench.setup_hints().len();
    bench.projection.record(
        &a_request_that_reached_a_browser(workspace.as_str()),
        Storage::Saved,
    );
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
    bench.projection.record(&again, Storage::Saved);
    assert_eq!(bench.setup_hints().len(), before + 1);
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

/// Protects desktop gate 3: global search finds destinations, sessions, operations, browsers,
/// recorded actions, diagnostics, configuration and supported clients by the words the window
/// shows for them, and opens the section that shows each (0fc62c90). A recorded action is found
/// by its tool, its sentence, the host it landed on, its agent, its browser and its outcome word,
/// newest first, never by an opaque id, and carries the outcome class its row draws, so a
/// credential refusal is Needs you, not Kept (2301b163 n-d-04, 3f27d340 V-05, 233e7569).
#[test]
fn the_search_finds_each_kind_by_the_words_its_row_shows_and_never_by_an_id() {
    use super::{SearchDestination, SearchKind};
    use crate::language::audit::AuditRefusal;
    use crate::language::history::OutcomeClass;
    use crate::language::workbench as words;

    let root = Scratch::named("search");
    let workspaces = crate::workspace::WorkspaceStore::default();
    let some = workspaces.admit("Some Agent".into(), IntakeChannel::Mcp);
    for unnamed in ["", "   "] {
        workspaces.admit(unnamed.into(), IntakeChannel::Mcp);
    }
    let projection = WorkbenchProjection::default();
    let facade = WorkbenchFacade::new(
        projection.clone(),
        workspaces,
        GovernanceFacade::new(None),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    )
    .with_harnesses(crate::install::HarnessRegistry::isolated(
        &root.join("clients"),
        &root.join("ghostlight"),
        &root.join("state"),
    ));
    facade.refresh_harnesses().unwrap();
    projection.react(&started(
        &client("Some Agent"),
        "invocation_1",
        some.as_str(),
        "browser_fill_form",
        PresentationActivity::Fill,
        Capability::Write,
    ));
    let find = |query: &str, kind: SearchKind| {
        facade
            .search(query)
            .into_iter()
            .find(|hit| hit.kind == kind)
            .unwrap_or_else(|| panic!("{query:?} finds a {kind:?}"))
    };

    for page in [
        words::SEARCH_GLANCE,
        words::SEARCH_INTEGRATIONS,
        words::SEARCH_STATUS,
        words::SEARCH_POLICY,
        words::SEARCH_SETTINGS,
    ] {
        let hit = find(page.title, SearchKind::Destination);
        assert_eq!((hit.id.as_str(), hit.title.as_str()), (page.id, page.title));
    }
    let session = find("some agent", SearchKind::Session);
    assert_eq!(session.detail, words::session_line(0, None, true, false));
    let unknown: Vec<_> = facade
        .search("unknown agent")
        .into_iter()
        .filter(|hit| hit.kind == SearchKind::Session)
        .map(|hit| hit.title)
        .collect();
    assert_eq!(
        unknown,
        [crate::language::provenance::UNKNOWN_AGENT; 2],
        "a session that sent no name is found by the name its card shows"
    );
    let operation = find("fill form", SearchKind::Operation);
    assert_eq!(operation.title, words::tool_words("browser_fill_form"));
    assert!(
        operation.detail.ends_with(words::SEARCH_RUNNING),
        "{}",
        operation.detail
    );
    assert!(
        !operation.detail.contains("workspace_"),
        "{}",
        operation.detail
    );
    let snapshot = facade.snapshot();
    let browser = &snapshot.browsers[0];
    assert_eq!(find(&browser.family, SearchKind::Browser).id, browser.id);
    assert_eq!(
        find(words::STATUS_RULES_LABEL, SearchKind::Diagnostic).id,
        AUTHORITY_CHECK
    );
    let row = &snapshot.harnesses[0];
    let installation = find(&row.label, SearchKind::Installation);
    assert_eq!(
        (installation.id.as_str(), installation.view),
        (row.id.as_str(), SearchDestination::Install)
    );
    let setting = find("browser work", SearchKind::Configuration);
    assert_eq!(setting.title, words::CONTROLS_RUNTIME_LABEL);
    assert_eq!(setting.detail, words::CONTROLS_RUNTIME_ACTIVE);
    // The window opens the section that shows the setting, not Settings as it was left.
    assert_eq!(setting.view, SearchDestination::BrowserWork);
    assert_eq!(serde_json::to_value(setting.view).unwrap(), "browser_work");

    let history = a_history_of(&[
        a_navigation_to("invocation_1", "example.com", 1_000),
        a_navigation_to("invocation_2", "sylin.org", 2_000),
    ]);
    let hits = history.search("SYLIN");
    assert_eq!(hits.len(), 1, "{hits:?}");
    let hit = &hits[0];
    assert_eq!(hit.kind, SearchKind::History);
    assert_eq!(hit.id, "invocation_2");
    assert_eq!(
        hit.title,
        words::search_action_title("browser_navigate", "Opened sylin.org.")
    );
    assert_eq!(hit.timestamp_ms, Some(2_000));
    assert_eq!(hit.outcome, None);
    assert!(hit.detail.ends_with(CHROME), "{}", hit.detail);
    assert!(!hit.detail.contains("workspace_"), "{}", hit.detail);
    let agent = crate::language::provenance::EARLIER_SESSION.to_ascii_lowercase();
    let chrome = CHROME.to_ascii_lowercase();
    for query in [
        agent.as_str(),
        "navigate",
        "browser_navigate",
        chrome.as_str(),
    ] {
        let found: Vec<String> = history
            .search(query)
            .into_iter()
            .filter(|hit| hit.kind == SearchKind::History)
            .map(|hit| hit.id)
            .collect();
        assert_eq!(found, ["invocation_2", "invocation_1"], "{query}");
    }
    for opaque in ["invocation_1", "workspace_1", "authority_1"] {
        assert!(history.search(opaque).is_empty(), "{opaque}");
    }

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
    let outcomes = a_history_of(&[failed, kept, blocked, credential]);
    assert_eq!(
        outcomes
            .search("example.com")
            .into_iter()
            .map(|hit| (hit.id, hit.outcome))
            .collect::<Vec<_>>(),
        [
            ("invocation_4".to_owned(), Some(OutcomeClass::NeedsYou)),
            ("invocation_3".to_owned(), Some(OutcomeClass::Refused)),
            ("invocation_2".to_owned(), Some(OutcomeClass::Kept)),
            ("invocation_1".to_owned(), Some(OutcomeClass::Failed)),
        ]
    );
    // Each is found by the word its row wears.
    for (class, invocation) in [
        (OutcomeClass::Refused, "invocation_3"),
        (OutcomeClass::NeedsYou, "invocation_4"),
        (OutcomeClass::Kept, "invocation_2"),
    ] {
        let word = words::search_outcome(class).expect("a class a row names");
        let found: Vec<String> = outcomes
            .search(word)
            .into_iter()
            .map(|hit| hit.id)
            .collect();
        assert_eq!(found, [invocation], "{word}");
    }
}

/// Guards the contract that keeps the window's live parts current (`WorkbenchChange::
/// refreshes_live_parts`; ACCEPTANCE Audit health 5): work that starts and saving health that
/// fails reach the band without a pull; the controls are pushed when they differ and not
/// otherwise; a repeated pause publishes no second status; and a policy hint is tagged with the
/// id of the authority now in force, so the window's one tag per part means one thing, and the
/// refresher does not repeat a hint the person's own act already gave.
#[test]
fn a_change_reaches_the_live_parts_without_a_pull_and_only_once() {
    use super::parts::HintedPart;
    use super::{RuntimeSource, WorkbenchRuntimeIntent};
    use crate::browser::adapter::{RuntimeControlIntent, RuntimeControlState};
    use crate::language::readiness::Readiness;

    let path = temporary_policy("hint");
    let governance = GovernanceFacade::owning_user_policy(path.to_path_buf());
    let projection = WorkbenchProjection::default();
    let facade = WorkbenchFacade::new(
        projection.clone(),
        crate::workspace::WorkspaceStore::default(),
        governance.clone(),
        ControlledBrowser::new(true, false),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let events = Arc::new(Events::default());
    facade.attach_events(events.clone());
    let controls = || {
        heard(&events)
            .into_iter()
            .filter_map(|event| match event.change {
                WorkbenchChange::ControlsChanged { controls } => Some(*controls),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let policy_hints = || {
        heard(&events)
            .into_iter()
            .filter_map(|event| match event.change {
                WorkbenchChange::PartChanged {
                    part: HintedPart::Policy,
                    etag,
                } => Some(etag),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    facade.refresh_live_parts();
    assert_eq!(controls().len(), 1, "the first look is always published");
    facade.refresh_live_parts();
    assert_eq!(controls().len(), 1, "a look that found nothing new is not");

    projection.react(&reading(&client("test"), "invocation_1", "workspace_1"));
    let working = statuses(&heard(&events)).pop().unwrap();
    assert_eq!(working.readiness.state, Readiness::Working);
    projection.audit_health_changed(failing_health(3));
    let health = statuses(&heard(&events)).pop().unwrap();
    assert_eq!(health.audit_health, failing_health(3));
    assert!(!health.audit_notice.is_empty());
    let before = heard(&events).len();
    facade.refresh_live_parts();
    facade.refresh_live_parts();
    assert_eq!(
        heard(&events).len(),
        before,
        "nothing changed, so a look says nothing"
    );

    facade.apply_runtime_intent(WorkbenchRuntimeIntent::Hold);
    let pushed = controls();
    assert_eq!(pushed.len(), 2);
    assert_eq!(pushed[1].runtime.state, RuntimeControlState::Held);
    assert_eq!(pushed[1], facade.snapshot().controls);
    let paused = statuses(&heard(&events)).len();
    let runtime_changes = || {
        heard(&events)
            .iter()
            .filter(|event| matches!(event.change, WorkbenchChange::RuntimeChanged { .. }))
            .count()
    };
    let changed = runtime_changes();
    facade.apply_runtime_control(RuntimeControlIntent::Hold, RuntimeSource::Tray);
    assert_eq!(
        statuses(&heard(&events)).len(),
        paused,
        "an identical pause published a second status"
    );
    // U-D4: a pause that changes nothing a screen shows publishes no runtime change either.
    assert_eq!(runtime_changes(), changed);

    let tag = super::parts::policy_etag(&facade.policy());
    facade.apply_user_policy_if(READ_ONLY_POLICY, &tag).unwrap();
    assert_eq!(policy_hints(), [governance.snapshot().id().to_owned()]);
    assert_eq!(policy_hints()[0], facade.policy().id);
    // The refresher has nothing new to say about an authority the person just applied.
    facade.authority_refreshed();
    assert_eq!(policy_hints().len(), 1);
    // A rules file edited by hand is news, and the hint names the authority now in force.
    std::fs::write(
        &path,
        READ_ONLY_POLICY.replace("\"name\":\"read\"", "\"name\":\"look-around\""),
    )
    .unwrap();
    governance.refresh_tick();
    facade.authority_refreshed();
    let edited = policy_hints();
    assert_eq!(edited.len(), 2);
    assert_eq!(edited[1], governance.snapshot().id());
    facade.authority_refreshed();
    assert_eq!(policy_hints().len(), 2);
    let tag = super::parts::policy_etag(&facade.policy());
    facade.remove_user_policy_if(&tag).unwrap();
    let removed = policy_hints();
    assert_eq!(removed.len(), 3);
    assert_eq!(removed[2], governance.snapshot().id());
}
