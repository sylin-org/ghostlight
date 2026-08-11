//! Closed in-process domain events for meaningful completed state changes.

use crate::governance::Capability;
use crate::workspace::TabHandle;
use ghostlight_bridge::browser::PresentationActivity;

/// Closed user-facing meaning for one blocked browser job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenialPresentation {
    /// A configured guardrail blocked an ordinary browser job.
    Guardrail,
    /// Policy kept a controlled tab visibly open.
    TabKeptOpenByPolicy,
    /// The browser-local preserve-tabs setting kept a controlled tab open.
    TabKeptOpenBySetting,
}

/// The complete in-process event vocabulary.
#[derive(Clone, Debug)]
pub enum DomainEvent {
    /// One unit of work started.
    WorkStarted {
        invocation: String,
        workspace: String,
        tool: String,
        activity: PresentationActivity,
        capability: Capability,
    },
    /// A controlled tab was created.
    TabCreated {
        invocation: String,
        workspace: String,
        tab: TabHandle,
        physical_id: u64,
    },
    /// A top-level document committed and passed landing governance.
    DocumentCommitted {
        invocation: String,
        workspace: String,
        tab: TabHandle,
        physical_id: u64,
    },
    /// A target is about to receive a governed physical action.
    TargetIndicated {
        invocation: String,
        workspace: String,
        physical_id: u64,
        locator: String,
    },
    /// One bounded sequence phase started.
    WorkPhaseStarted {
        invocation: String,
        workspace: String,
        physical_id: Option<u64>,
        activity: PresentationActivity,
    },
    /// A tab entered runtime hold.
    HoldEntered {
        invocation: String,
        workspace: String,
        physical_id: u64,
    },
    /// Visible user attention is required.
    AttentionRequired {
        invocation: String,
        workspace: String,
        physical_id: Option<u64>,
    },
    /// Authority blocked work.
    WorkBlocked {
        invocation: String,
        workspace: String,
        physical_id: Option<u64>,
        presentation: DenialPresentation,
    },
    /// Work reached its only terminal completion.
    WorkCompleted {
        invocation: String,
        workspace: String,
        physical_id: Option<u64>,
    },
}

impl DomainEvent {
    /// Opaque invocation handle shared by all event reactions.
    #[must_use]
    pub fn invocation(&self) -> &str {
        match self {
            Self::WorkStarted { invocation, .. }
            | Self::TabCreated { invocation, .. }
            | Self::DocumentCommitted { invocation, .. }
            | Self::TargetIndicated { invocation, .. }
            | Self::WorkPhaseStarted { invocation, .. }
            | Self::HoldEntered { invocation, .. }
            | Self::AttentionRequired { invocation, .. }
            | Self::WorkBlocked { invocation, .. }
            | Self::WorkCompleted { invocation, .. } => invocation,
        }
    }

    /// Opaque workspace handle shared by all event reactions.
    #[must_use]
    pub fn workspace(&self) -> &str {
        match self {
            Self::WorkStarted { workspace, .. }
            | Self::TabCreated { workspace, .. }
            | Self::DocumentCommitted { workspace, .. }
            | Self::TargetIndicated { workspace, .. }
            | Self::WorkPhaseStarted { workspace, .. }
            | Self::HoldEntered { workspace, .. }
            | Self::AttentionRequired { workspace, .. }
            | Self::WorkBlocked { workspace, .. }
            | Self::WorkCompleted { workspace, .. } => workspace,
        }
    }
}
