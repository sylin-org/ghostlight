//! Content-free user feedback reactions and their policy-free browser port.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, PresentationActivity, PresentationKind, PresentationSignal,
};
use thiserror::Error;

use crate::browser::{addressed_tab, BrowserError, BrowserPort};
use crate::events::DomainEvent;
use crate::language::presentation as presentation_words;
use crate::sync::lock;
use crate::workspace::WorkspaceStore;

/// How long one presentation may take, including the tab lookup a protocol-4 notice needs.
const PRESENTATION_BUDGET: Duration = Duration::from_millis(500);

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
    ///
    /// The browser draws a notice only on the tab it names. A denial signal that
    /// names no tab is given the workspace's only tab, or its active one.
    fn present(
        &self,
        workspace: &str,
        mut signal: PresentationSignal,
    ) -> Result<(), PresentationError> {
        let Some(browser) = self.workspaces.browser_of(workspace) else {
            return Ok(());
        };
        let deadline = Instant::now() + PRESENTATION_BUDGET;
        let cancelled = AtomicBool::new(false);
        if signal.tab_id.is_none() && signal.signal == PresentationKind::Denial {
            signal.tab_id = self.notice_tab(&browser, workspace, deadline, &cancelled);
        }
        self.browser
            .call(
                &browser,
                BrowserCommand::Present { signal },
                deadline,
                &cancelled,
            )
            .map(|_| ())
            .map_err(PresentationError::Browser)
    }
}

impl BrowserPresentation {
    /// The tab a workspace-wide notice belongs on: the workspace's only tab, or the active one
    /// among its tabs as one tab listing reports it. None when neither is known.
    fn notice_tab(
        &self,
        browser: &str,
        workspace: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<u64> {
        let tabs = self.workspaces.physical_tabs(workspace);
        match tabs.as_slice() {
            [] => None,
            [only] => Some(*only),
            several => {
                match self
                    .browser
                    .call(browser, BrowserCommand::ListTabs, deadline, cancelled)
                {
                    Ok(BrowserOutcome::Tabs { tabs: listed }) => listed
                        .iter()
                        .find(|tab| tab.active && several.contains(&tab.tab_id))
                        .map(|tab| tab.tab_id),
                    _ => None,
                }
            }
        }
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
            lock(&self.activities)
                .get(event.invocation())
                .map(|active| active.activity)
                .unwrap_or(PresentationActivity::Quiet)
        };
        let recorded_tab = || {
            lock(&self.activities)
                .get(event.invocation())
                .and_then(|active| active.tab)
        };
        let (signal, activity, phase, detail, tab_id, locator, terminal) = match event {
            DomainEvent::WorkWaiting { .. } => return,
            DomainEvent::WorkStarted { activity, .. } => {
                lock(&self.activities).insert(
                    event.invocation().into(),
                    ActivePresentation {
                        activity: *activity,
                        tab: None,
                    },
                );
                (
                    PresentationKind::Start,
                    *activity,
                    presentation_words::activity_label(*activity),
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
                lock(&self.activities).insert(
                    event.invocation().into(),
                    ActivePresentation {
                        activity: *activity,
                        tab: *physical_id,
                    },
                );
                (
                    PresentationKind::Start,
                    *activity,
                    presentation_words::activity_label(*activity),
                    None,
                    *physical_id,
                    None,
                    false,
                )
            }
            DomainEvent::TabCreated { physical_id, .. } => (
                PresentationKind::Progress,
                current(),
                presentation_words::activity_label(current()),
                None,
                Some(*physical_id),
                None,
                false,
            ),
            DomainEvent::DocumentCommitted { physical_id, .. } => (
                PresentationKind::Progress,
                current(),
                presentation_words::activity_label(current()),
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
                presentation_words::activity_label(current()),
                None,
                Some(*physical_id),
                Some(locator.clone()),
                false,
            ),
            DomainEvent::HoldEntered { physical_id, .. } => (
                PresentationKind::Denial,
                current(),
                presentation_words::HOLD_PHASE,
                Some(presentation_words::HOLD_DETAIL),
                Some(*physical_id),
                None,
                false,
            ),
            DomainEvent::WorkBlocked {
                physical_id,
                presentation,
                ..
            } => {
                let (phase, detail) = presentation_words::denial_lines(*presentation);
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
                presentation_words::activity_label(current()),
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
            label: presentation_words::activity_label(activity).into(),
            detail: detail.map(str::to_owned),
            tab_id,
            locator,
            click,
        };
        let _ = self.port.present(event.workspace(), frame);
        if terminal {
            lock(&self.activities).remove(event.invocation());
        }
    }

    /// Bind page feedback to the physical command's tab, once per invocation phase.
    ///
    /// Admission has no page destination yet. Recording a workspace-wide start must never
    /// animate whichever unrelated tab happens to be active while the real target resolves.
    pub(crate) fn bind_command(&self, workspace: &str, invocation: &str, command: &BrowserCommand) {
        let Some(tab) = addressed_tab(command) else {
            return;
        };
        let activity = {
            let mut activities = lock(&self.activities);
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
                phase: presentation_words::activity_label(activity).into(),
                label: presentation_words::activity_label(activity).into(),
                detail: None,
                tab_id: Some(tab),
                locator: None,
                click: None,
            },
        );
    }

    /// Bind page feedback to `tab` without starting the activity there, for an action whose one
    /// cue is the target it names once the browser has acted. A command on that tab then starts
    /// nothing, so a scroll to a target draws no cue at the viewport centre before its own.
    pub(crate) fn bind_for_target(&self, invocation: &str, tab: u64) {
        if let Some(active) = lock(&self.activities).get_mut(invocation) {
            active.tab = Some(tab);
        }
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

    use crate::browser::adapter::{PresentationActivity, PresentationKind, PresentationSignal};

    use crate::events::DomainEvent;
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

    /// Defect 27199af1 ("isolate page feedback", the security-hardening regression suite's page
    /// feedback row): two reads in flight in one workspace each draw on their own resolved tab.
    /// Admission invents no page, each read starts once on the tab its command names, its
    /// completion falls back only to its own destination, and neither one's completion clears the
    /// other's progress.
    #[test]
    fn overlapping_reads_keep_their_page_destination_and_completion_independent() {
        use crate::browser::adapter::BrowserCommand;

        let port = Arc::new(RecordingPort::default());
        let reactor = PresentationReactor::new(port.clone());
        for (invocation, tab) in [("read-a", 7), ("read-b", 11)] {
            reactor.react(&DomainEvent::WorkStarted {
                invocation: invocation.into(),
                workspace: "shared-workspace".into(),
                tool: "browser_read".into(),
                activity: PresentationActivity::Read,
                capabilities: Capability::Read.into(),
                provenance: crate::provenance::ConnectionEvidence::claimed(
                    "test",
                    crate::intake::session::IntakeChannel::Mcp,
                )
                .details(),
                received_at_ms: crate::audit::unix_ms(),
            });
            let command = BrowserCommand::ReadDocument {
                tab_id: tab,
                mode: crate::language::ReadMode::Visible,
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
}
