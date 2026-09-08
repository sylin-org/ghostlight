//! Content-free user feedback reactions and their policy-free browser port.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ghostlight_bridge::browser::{
    BrowserCommand, PresentationActivity, PresentationKind, PresentationSignal,
};
use thiserror::Error;

use crate::browser::{BrowserError, BrowserPort};
use crate::events::{DenialPresentation, DomainEvent};
use crate::workspace::WorkspaceStore;

/// Content-free presentation output port.
pub trait PresentationPort: Send + Sync {
    /// Render one fixed signal. Failure is never product failure.
    fn present(&self, workspace: &str, signal: PresentationSignal)
        -> Result<(), PresentationError>;
}

/// Physical adapter-backed presentation port.
pub struct BrowserPresentation {
    browser: Arc<dyn BrowserPort>,
    workspaces: WorkspaceStore,
}

impl BrowserPresentation {
    /// Construct presentation over the physical browser port.
    #[must_use]
    pub fn new(browser: Arc<dyn BrowserPort>, workspaces: WorkspaceStore) -> Self {
        Self {
            browser,
            workspaces,
        }
    }
}

impl PresentationPort for BrowserPresentation {
    /// Present into the browser the workspace already works in, and nowhere else.
    ///
    /// Presentation follows work; it never leads it. A workspace that has not chosen a browser has
    /// nothing to show, and showing its signal in some other browser would put a stranger's window
    /// under Ghostlight's badge.
    fn present(
        &self,
        workspace: &str,
        signal: PresentationSignal,
    ) -> Result<(), PresentationError> {
        let Some(browser) = self.workspaces.browser_of(workspace) else {
            return Ok(());
        };
        let cancelled = AtomicBool::new(false);
        self.browser
            .call(
                &browser,
                workspace,
                BrowserCommand::Present { signal },
                Instant::now() + Duration::from_millis(500),
                &cancelled,
            )
            .map(|_| ())
            .map_err(PresentationError::Browser)
    }
}

/// Direct typed domain-event reaction for presentation.
#[derive(Clone)]
pub struct PresentationReactor {
    port: Arc<dyn PresentationPort>,
    activities: Arc<Mutex<HashMap<String, ActivePresentation>>>,
}

#[derive(Clone, Copy)]
struct ActivePresentation {
    activity: PresentationActivity,
    tab: Option<u64>,
}

impl PresentationReactor {
    /// Construct the reactor.
    #[must_use]
    pub fn new(port: Arc<dyn PresentationPort>) -> Self {
        Self {
            port,
            activities: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// React synchronously and intentionally ignore presentation failure.
    pub fn react(&self, event: &DomainEvent) {
        let current = || {
            self.activities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(event.invocation())
                .map(|active| active.activity)
                .unwrap_or(PresentationActivity::Quiet)
        };
        let recorded_tab = || {
            self.activities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(event.invocation())
                .and_then(|active| active.tab)
        };
        let (signal, activity, phase, detail, tab_id, locator, terminal) = match event {
            DomainEvent::WorkWaiting { .. } => return,
            DomainEvent::WorkStarted { activity, .. } => {
                self.activities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        event.invocation().into(),
                        ActivePresentation {
                            activity: *activity,
                            tab: None,
                        },
                    );
                (
                    PresentationKind::Start,
                    *activity,
                    activity_label(*activity),
                    None,
                    None,
                    None,
                    false,
                )
            }
            DomainEvent::WorkPhaseStarted {
                activity,
                physical_id,
                ..
            } => {
                self.activities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        event.invocation().into(),
                        ActivePresentation {
                            activity: *activity,
                            tab: *physical_id,
                        },
                    );
                (
                    PresentationKind::Start,
                    *activity,
                    activity_label(*activity),
                    None,
                    *physical_id,
                    None,
                    false,
                )
            }
            DomainEvent::TabCreated { physical_id, .. } => (
                PresentationKind::Progress,
                current(),
                activity_label(current()),
                None,
                Some(*physical_id),
                None,
                false,
            ),
            DomainEvent::DocumentCommitted { physical_id, .. } => (
                PresentationKind::Progress,
                current(),
                activity_label(current()),
                None,
                Some(*physical_id),
                None,
                false,
            ),
            DomainEvent::TargetIndicated {
                physical_id,
                locator,
                ..
            } => (
                PresentationKind::Target,
                current(),
                activity_label(current()),
                None,
                Some(*physical_id),
                Some(locator.clone()),
                false,
            ),
            DomainEvent::HoldEntered { physical_id, .. } => (
                PresentationKind::Denial,
                current(),
                "Ghostlight held this action",
                Some("A runtime guardrail paused browser work."),
                Some(*physical_id),
                None,
                false,
            ),
            DomainEvent::AttentionRequired { physical_id, .. } => (
                PresentationKind::Attention,
                current(),
                "Ghostlight needs your attention",
                None,
                physical_id.or_else(recorded_tab),
                None,
                true,
            ),
            DomainEvent::WorkBlocked {
                physical_id,
                presentation,
                ..
            } => {
                let (phase, detail) = denial_copy(*presentation);
                (
                    PresentationKind::Denial,
                    current(),
                    phase,
                    Some(detail),
                    physical_id.or_else(recorded_tab),
                    None,
                    true,
                )
            }
            DomainEvent::WorkCompleted { physical_id, .. } => (
                PresentationKind::Completion,
                current(),
                activity_label(current()),
                None,
                physical_id.or_else(recorded_tab),
                None,
                true,
            ),
        };
        // Only a click describes its own shape, so the confirmation can match what landed.
        let click = match event {
            DomainEvent::TargetIndicated { click, .. } => click.clone(),
            _ => None,
        };
        let frame = PresentationSignal {
            invocation: event.invocation().into(),
            signal,
            activity,
            phase: phase.into(),
            detail: detail.map(str::to_owned),
            tab_id,
            locator,
            click,
        };
        let _ = self.port.present(event.workspace(), frame);
        if terminal {
            self.activities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(event.invocation());
        }
    }

    /// Bind page feedback to the physical command's tab, once per invocation phase.
    ///
    /// Admission has no page destination yet. Recording a workspace-wide start must never
    /// animate whichever unrelated tab happens to be active while the real target resolves.
    pub(crate) fn bind_command(&self, workspace: &str, invocation: &str, command: &BrowserCommand) {
        let Some(tab) = command_tab(command) else {
            return;
        };
        let activity = {
            let mut activities = self
                .activities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(active) = activities.get_mut(invocation) else {
                return;
            };
            if active.tab == Some(tab) {
                return;
            }
            active.tab = Some(tab);
            active.activity
        };
        let _ = self.port.present(
            workspace,
            PresentationSignal {
                invocation: invocation.into(),
                signal: PresentationKind::Start,
                activity,
                phase: activity_label(activity).into(),
                detail: None,
                tab_id: Some(tab),
                locator: None,
                click: None,
            },
        );
    }
}

fn command_tab(command: &BrowserCommand) -> Option<u64> {
    match command {
        BrowserCommand::InDocuments { primitive, .. } => command_tab(primitive),
        BrowserCommand::DescribeDocuments { tab_id, .. }
        | BrowserCommand::FocusTab { tab_id }
        | BrowserCommand::Navigate { tab_id, .. }
        | BrowserCommand::TraverseHistory { tab_id, .. }
        | BrowserCommand::Reload { tab_id, .. }
        | BrowserCommand::CloseTab { tab_id, .. }
        | BrowserCommand::NavigateDiscardingBeforeUnload { tab_id, .. }
        | BrowserCommand::ReadText { tab_id, .. }
        | BrowserCommand::ReadDocument { tab_id, .. }
        | BrowserCommand::Inspect { tab_id, .. }
        | BrowserCommand::InspectTree { tab_id, .. }
        | BrowserCommand::Find { tab_id, .. }
        | BrowserCommand::Screenshot { tab_id, .. }
        | BrowserCommand::ScreenshotRegion { tab_id, .. }
        | BrowserCommand::DescribeTargets { tab_id, .. }
        | BrowserCommand::QuerySemantic { tab_id, .. }
        | BrowserCommand::Activate { tab_id, .. }
        | BrowserCommand::ActivatePoint { tab_id, .. }
        | BrowserCommand::ActivateModified { tab_id, .. }
        | BrowserCommand::ActivatePointModified { tab_id, .. }
        | BrowserCommand::WheelAt { tab_id, .. }
        | BrowserCommand::Scroll { tab_id, .. }
        | BrowserCommand::SetZoom { tab_id, .. }
        | BrowserCommand::ResizeWindow { tab_id, .. }
        | BrowserCommand::Hover { tab_id, .. }
        | BrowserCommand::HoverPoint { tab_id, .. }
        | BrowserCommand::Fill { tab_id, .. }
        | BrowserCommand::TypeText { tab_id, .. }
        | BrowserCommand::DescribeFocused { tab_id }
        | BrowserCommand::TypeFocused { tab_id, .. }
        | BrowserCommand::PressKey { tab_id, .. }
        | BrowserCommand::Drag { tab_id, .. }
        | BrowserCommand::DragPoints { tab_id, .. }
        | BrowserCommand::UploadFiles { tab_id, .. }
        | BrowserCommand::DropImageAt { tab_id, .. }
        | BrowserCommand::EvaluateScript { tab_id, .. }
        | BrowserCommand::Observe { tab_id, .. }
        | BrowserCommand::InspectDialog { tab_id }
        | BrowserCommand::HandleDialog { tab_id, .. }
        | BrowserCommand::ReadDiagnostics { tab_id, .. }
        | BrowserCommand::StartRecording { tab_id } => Some(*tab_id),
        BrowserCommand::ListTabs
        | BrowserCommand::OpenTab { .. }
        | BrowserCommand::ClearDiagnostics { .. }
        | BrowserCommand::StatusRecording { .. }
        | BrowserCommand::StopRecording { .. }
        | BrowserCommand::ExportRecording { .. }
        | BrowserCommand::DiscardRecording { .. }
        | BrowserCommand::Cancel { .. }
        | BrowserCommand::Present { .. } => None,
    }
}

fn denial_copy(presentation: DenialPresentation) -> (&'static str, &'static str) {
    match presentation {
        DenialPresentation::Guardrail => (
            "Ghostlight blocked this action",
            "A configured guardrail prevented it.",
        ),
        DenialPresentation::TabKeptOpenByPolicy => (
            "Ghostlight kept this tab open",
            "Closing tabs is blocked by policy. You can close it yourself.",
        ),
        DenialPresentation::TabKeptOpenBySetting => (
            "Ghostlight kept this tab open",
            "Your Preserve Ghostlight tabs setting is on. You can close it yourself.",
        ),
    }
}

fn activity_label(activity: PresentationActivity) -> &'static str {
    match activity {
        PresentationActivity::Quiet => "Ghostlight",
        PresentationActivity::Navigate => "Navigating",
        PresentationActivity::Click => "Clicking",
        PresentationActivity::Hover => "Hovering",
        PresentationActivity::Drag => "Dragging",
        PresentationActivity::Type => "Typing",
        PresentationActivity::Key => "Keyboard",
        PresentationActivity::Scroll => "Scrolling",
        PresentationActivity::Read => "Reading page",
        PresentationActivity::Find => "Finding on page",
        PresentationActivity::Screenshot => "Screenshot",
        PresentationActivity::Zoom => "Zooming",
        PresentationActivity::Fill => "Filling form",
        PresentationActivity::Upload => "Uploading file",
        PresentationActivity::Script => "Running JavaScript",
        PresentationActivity::Wait => "Waiting",
        PresentationActivity::Dialog => "Browser dialog",
    }
}

/// Presentation-only failure that cannot affect authority or completion.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PresentationError {
    /// The physical adapter could not render feedback.
    #[error("browser presentation failed: {0}")]
    Browser(BrowserError),
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ghostlight_bridge::browser::{PresentationActivity, PresentationKind, PresentationSignal};

    use crate::events::{DenialPresentation, DomainEvent};
    use crate::governance::Capability;

    use super::{PresentationError, PresentationPort, PresentationReactor};

    #[derive(Default)]
    struct RecordingPort(Mutex<Vec<PresentationSignal>>);

    impl PresentationPort for RecordingPort {
        fn present(
            &self,
            _workspace: &str,
            signal: PresentationSignal,
        ) -> Result<(), PresentationError> {
            self.0.lock().unwrap().push(signal);
            Ok(())
        }
    }

    struct FailingPort;
    impl PresentationPort for FailingPort {
        fn present(
            &self,
            _workspace: &str,
            _signal: PresentationSignal,
        ) -> Result<(), PresentationError> {
            Err(PresentationError::Browser(
                crate::browser::BrowserError::DisconnectedBeforeDispatch,
            ))
        }
    }

    #[test]
    fn presentation_contains_only_content_free_fixed_fields() {
        let port = Arc::new(RecordingPort::default());
        let reactor = PresentationReactor::new(port.clone());
        reactor.react(&DomainEvent::WorkStarted {
            provenance: None,
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            tool: "browser_read".into(),
            activity: PresentationActivity::Read,
            capabilities: Capability::Read.into(),
        });
        reactor.react(&DomainEvent::TargetIndicated {
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            physical_id: 7,
            locator: "locator_1".into(),
            click: None,
        });
        let signals = port.0.lock().unwrap();
        assert_eq!(signals[0].phase, "Reading page");
        assert_eq!(signals[1].activity, PresentationActivity::Read);
        let json = serde_json::to_string(&signals[0]).unwrap();
        assert!(!json.contains("url"));
        assert!(!json.contains("content"));
    }

    #[test]
    fn a_click_describes_its_shape_and_nothing_else_does() {
        use ghostlight_bridge::browser::ClickShape;

        let port = Arc::new(RecordingPort::default());
        let reactor = PresentationReactor::new(port.clone());
        reactor.react(&DomainEvent::TargetIndicated {
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            physical_id: 7,
            locator: "locator_1".into(),
            click: Some(ClickShape {
                clicks: 2,
                button: "secondary".into(),
            }),
        });
        reactor.react(&DomainEvent::TargetIndicated {
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            physical_id: 7,
            locator: "locator_2".into(),
            click: None,
        });

        let signals = port.0.lock().unwrap();
        let shape = signals[0].click.as_ref().expect("a click describes itself");
        assert_eq!(shape.clicks, 2);
        assert_eq!(shape.button, "secondary");
        assert!(
            signals[1].click.is_none(),
            "a hover, drag, or type indication has no click to describe"
        );

        // The shape says how the click landed, never what the page holds. The locator is a
        // legitimate indication field and is expected here.
        let encoded = serde_json::to_string(&signals[0]).unwrap();
        for forbidden in ["url", "content", "password", "text"] {
            assert!(!encoded.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn presentation_failure_is_non_authoritative() {
        let reactor = PresentationReactor::new(Arc::new(FailingPort));
        reactor.react(&DomainEvent::WorkCompleted {
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            physical_id: None,
        });
    }

    #[test]
    fn tab_close_policy_denial_names_the_visible_outcome_and_source() {
        let port = Arc::new(RecordingPort::default());
        let reactor = PresentationReactor::new(port.clone());
        reactor.react(&DomainEvent::WorkStarted {
            provenance: None,
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            tool: "browser_tabs".into(),
            activity: PresentationActivity::Quiet,
            capabilities: Capability::Action.into(),
        });
        reactor.react(&DomainEvent::WorkBlocked {
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            physical_id: Some(7),
            presentation: DenialPresentation::TabKeptOpenByPolicy,
        });
        let signals = port.0.lock().unwrap();
        assert_eq!(signals[1].signal, PresentationKind::Denial);
        assert_eq!(signals[1].tab_id, Some(7));
        assert_eq!(signals[1].phase, "Ghostlight kept this tab open");
        assert_eq!(
            signals[1].detail.as_deref(),
            Some("Closing tabs is blocked by policy. You can close it yourself.")
        );
    }

    #[test]
    fn tab_close_local_denial_names_the_setting() {
        let port = Arc::new(RecordingPort::default());
        let reactor = PresentationReactor::new(port.clone());
        reactor.react(&DomainEvent::WorkBlocked {
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            physical_id: Some(7),
            presentation: DenialPresentation::TabKeptOpenBySetting,
        });
        let signals = port.0.lock().unwrap();
        assert_eq!(signals[0].signal, PresentationKind::Denial);
        assert_eq!(signals[0].phase, "Ghostlight kept this tab open");
        assert_eq!(
            signals[0].detail.as_deref(),
            Some("Your Preserve Ghostlight tabs setting is on. You can close it yourself.")
        );
    }

    #[test]
    fn overlapping_reads_keep_their_page_destination_and_completion_independent() {
        use ghostlight_bridge::browser::BrowserCommand;

        let port = Arc::new(RecordingPort::default());
        let reactor = PresentationReactor::new(port.clone());
        for (invocation, tab) in [("read-a", 7), ("read-b", 11)] {
            reactor.react(&DomainEvent::WorkStarted {
                invocation: invocation.into(),
                workspace: "shared-workspace".into(),
                tool: "browser_read".into(),
                activity: PresentationActivity::Read,
                capabilities: Capability::Read.into(),
                provenance: None,
            });
            let command = BrowserCommand::ReadDocument {
                tab_id: tab,
                mode: "visible".into(),
                max_chars: 500,
            };
            reactor.bind_command("shared-workspace", invocation, &command);
            reactor.bind_command("shared-workspace", invocation, &command);
        }
        reactor.react(&DomainEvent::WorkCompleted {
            invocation: "read-a".into(),
            workspace: "shared-workspace".into(),
            physical_id: None,
        });
        assert_eq!(
            reactor
                .activities
                .lock()
                .unwrap()
                .get("read-b")
                .unwrap()
                .tab,
            Some(11)
        );
        reactor.react(&DomainEvent::WorkCompleted {
            invocation: "read-b".into(),
            workspace: "shared-workspace".into(),
            physical_id: None,
        });
        let signals = port.0.lock().unwrap();
        for (invocation, tab) in [("read-a", 7), ("read-b", 11)] {
            let signals: Vec<_> = signals
                .iter()
                .filter(|signal| signal.invocation == invocation)
                .collect();
            assert_eq!(
                signals.len(),
                3,
                "one admission, one exact-page start, one completion"
            );
            assert_eq!(signals[0].tab_id, None, "admission cannot invent a page");
            assert_eq!(signals[1].signal, PresentationKind::Start);
            assert_eq!(signals[1].tab_id, Some(tab));
            assert_eq!(signals[2].signal, PresentationKind::Completion);
            assert_eq!(
                signals[2].tab_id,
                Some(tab),
                "a missing terminal tab falls back only to this invocation's destination"
            );
        }
        assert!(reactor.activities.lock().unwrap().is_empty());
    }

    #[test]
    fn composition_phases_bind_fresh_destinations_and_denials_finish_their_origin() {
        use ghostlight_bridge::browser::BrowserCommand;

        let port = Arc::new(RecordingPort::default());
        let reactor = PresentationReactor::new(port.clone());
        for (activity, tab) in [
            (PresentationActivity::Script, 7),
            (PresentationActivity::Read, 11),
        ] {
            reactor.react(&DomainEvent::WorkPhaseStarted {
                invocation: "flow".into(),
                workspace: "shared-workspace".into(),
                physical_id: None,
                activity,
            });
            reactor.bind_command(
                "shared-workspace",
                "flow",
                &BrowserCommand::DescribeDocuments {
                    tab_id: tab,
                    locators: vec![],
                    points: vec![],
                    focused: false,
                },
            );
            reactor.react(&DomainEvent::WorkBlocked {
                invocation: "flow".into(),
                workspace: "shared-workspace".into(),
                physical_id: None,
                presentation: DenialPresentation::Guardrail,
            });
        }
        let signals = port.0.lock().unwrap();
        for (phase, tab) in signals.chunks_exact(3).zip([7, 11]) {
            assert_eq!(phase[0].tab_id, None);
            assert_eq!(phase[1].tab_id, Some(tab));
            assert_eq!(phase[2].tab_id, Some(tab));
            assert_eq!(phase[2].signal, PresentationKind::Denial);
            assert_eq!(phase[2].activity, phase[1].activity);
        }
        assert!(reactor.activities.lock().unwrap().is_empty());
    }
}
