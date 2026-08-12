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

/// Content-free presentation output port.
pub trait PresentationPort: Send + Sync {
    /// Render one fixed signal. Failure is never product failure.
    fn present(&self, workspace: &str, signal: PresentationSignal)
        -> Result<(), PresentationError>;
}

/// Physical adapter-backed presentation port.
pub struct BrowserPresentation {
    browser: Arc<dyn BrowserPort>,
}

impl BrowserPresentation {
    /// Construct presentation over the physical browser port.
    #[must_use]
    pub fn new(browser: Arc<dyn BrowserPort>) -> Self {
        Self { browser }
    }
}

impl PresentationPort for BrowserPresentation {
    fn present(
        &self,
        workspace: &str,
        signal: PresentationSignal,
    ) -> Result<(), PresentationError> {
        let cancelled = AtomicBool::new(false);
        self.browser
            .call(
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
    activities: Arc<Mutex<HashMap<String, PresentationActivity>>>,
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
                .copied()
                .unwrap_or(PresentationActivity::Quiet)
        };
        let (signal, activity, phase, detail, tab_id, locator, terminal) = match event {
            DomainEvent::WorkStarted { activity, .. } => {
                self.activities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(event.invocation().into(), *activity);
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
                    .insert(event.invocation().into(), *activity);
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
                *physical_id,
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
                    *physical_id,
                    None,
                    true,
                )
            }
            DomainEvent::WorkCompleted { physical_id, .. } => (
                PresentationKind::Completion,
                current(),
                activity_label(current()),
                None,
                *physical_id,
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
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            tool: "browser_read".into(),
            activity: PresentationActivity::Read,
            capability: Capability::Read,
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
            invocation: "invocation_x".into(),
            workspace: "workspace_x".into(),
            tool: "browser_tabs".into(),
            activity: PresentationActivity::Quiet,
            capability: Capability::Action,
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
}
