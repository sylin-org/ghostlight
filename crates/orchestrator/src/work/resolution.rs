//! Work-owned attempt evidence and immutable action resolution, independent of rendered receipts.

use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome};
use ghostlight_bridge::service::ServiceContent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::browser::BrowserError;
use crate::governance::Decision;
use crate::language::coverage::Coverage;
use crate::language::outcome::{Observed, Outcome, Refusal};

use super::result::{Effect, Readiness, Status};

/// Which responsibility supplied failure evidence, relative to the requested action.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    #[default]
    Admission,
    Preparation,
    RequestedEffect,
    Verification,
    Compensation,
}

/// Bounded mechanical progress; no value, script, page text or expected condition is retained.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Progress {
    pub attempted: u32,
    pub acknowledged: u32,
    pub confirmed_effects: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<u32>,
}

/// Mutable evidence belongs only to the invocation currently executing, never to history.
#[derive(Debug, Default)]
pub(super) struct ExecutionEvidence {
    phase: Phase,
    progress: Progress,
    uncertain: bool,
    pending_effects: u32,
}

impl ExecutionEvidence {
    /// Declare the bounded number of requested physical actions in this invocation.
    pub(super) fn expect(&mut self, count: usize) {
        self.progress.expected = Some(u32::try_from(count).unwrap_or(u32::MAX));
    }

    /// Retain physical attempt evidence without confirming identity or document scope.
    pub(super) fn record(
        &mut self,
        phase: Phase,
        command: &BrowserCommand,
        result: &Result<BrowserOutcome, BrowserError>,
    ) {
        let reported = match result {
            Ok(outcome) => super::reported_failure(outcome).map(Err),
            Err(_) => None,
        };
        let result = reported.as_ref().unwrap_or(result);
        let phase = match command {
            BrowserCommand::DescribeDocuments { .. }
            | BrowserCommand::DescribeTargets { .. }
            | BrowserCommand::DescribeFocused { .. }
            | BrowserCommand::InstallPageRuntime { .. } => Phase::Preparation,
            _ => phase,
        };
        match result {
            Ok(outcome) if compatible(command, outcome) => {
                self.phase = phase;
                if phase == Phase::RequestedEffect {
                    self.progress.attempted = self.progress.attempted.saturating_add(1);
                    self.progress.acknowledged = self.progress.acknowledged.saturating_add(1);
                    if mutates(command) && acknowledges_effect(outcome) {
                        self.pending_effects = self.pending_effects.saturating_add(1);
                    }
                }
            }
            Ok(BrowserOutcome::AttentionProtected { .. }) => self.phase = phase,
            Ok(_) => {
                self.phase = phase;
                if phase == Phase::RequestedEffect {
                    self.progress.attempted = self.progress.attempted.saturating_add(1);
                    self.uncertain = true;
                }
            }
            Err(error) => {
                self.phase = phase;
                if phase == Phase::RequestedEffect && error.effect_unknown() {
                    self.progress.attempted = self.progress.attempted.saturating_add(1);
                    self.uncertain = true;
                }
            }
        }
    }

    /// Confirm effects only after the requested identity and admitted document receipt validate.
    pub(super) fn confirm(
        &mut self,
        phase: Phase,
        command: &BrowserCommand,
        outcome: &BrowserOutcome,
    ) {
        if phase == Phase::RequestedEffect && mutates(command) && acknowledges_effect(outcome) {
            self.pending_effects = self.pending_effects.saturating_sub(1);
            self.progress.confirmed_effects = self.progress.confirmed_effects.saturating_add(1);
        }
    }

    /// A received but incompatible requested receipt cannot establish the action's effects.
    pub(super) fn incompatible(&mut self, phase: Phase) {
        self.phase = phase;
        if phase == Phase::RequestedEffect {
            self.uncertain = true;
        }
    }
}

/// Declared verification is separate from what the requested action already did.
#[derive(Debug)]
pub(crate) enum Verification {
    NotRequested,
    Met,
    NotMet,
    Unavailable(Refusal),
}

/// Typed operation meaning. Strings are authored only later by Language.
#[derive(Debug)]
pub(crate) enum Conclusion {
    Outcome(Outcome),
    Refusal(Refusal),
}

/// Transient client payload; it is never a source for retained audit or human language.
#[derive(Debug)]
pub(crate) struct ClientPayload {
    pub facts: Value,
    pub content: Vec<ServiceContent>,
    pub guidance: Vec<String>,
}

/// Operation meaning awaiting the invocation's final evidence, with no caller/audit rendering.
#[derive(Debug)]
pub(super) struct WorkEvidence {
    invocation: String,
    disposition: Status,
    declared_effect: Effect,
    readiness: Readiness,
    repeat_candidate: bool,
    conclusion: Conclusion,
    verification: Verification,
    pub(super) decision: Decision,
    pub(super) physical_id: Option<u64>,
    pub(super) payload: ClientPayload,
    pub(super) tools: Vec<String>,
    pub(super) unconfirmed_history_steps: u32,
}

impl WorkEvidence {
    /// Correlate transient evidence before the action account is consumed.
    pub(super) fn invocation(&self) -> &str {
        &self.invocation
    }
    /// Begin typed operation evidence without rendering a caller or retained receipt.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        invocation: &str,
        disposition: Status,
        declared_effect: Effect,
        readiness: Readiness,
        repeat_candidate: bool,
        conclusion: Conclusion,
        facts: Value,
        decision: Decision,
        physical_id: Option<u64>,
    ) -> Self {
        Self {
            invocation: invocation.into(),
            disposition,
            declared_effect,
            readiness,
            repeat_candidate,
            conclusion,
            verification: Verification::NotRequested,
            decision,
            physical_id,
            payload: ClientPayload {
                facts,
                content: Vec::new(),
                guidance: Vec::new(),
            },
            tools: Vec::new(),
            unconfirmed_history_steps: 0,
        }
    }

    /// Whether the operation body completed before separate declared verification.
    pub(super) fn completed(&self) -> bool {
        self.disposition == Status::Succeeded
    }

    /// Attach the explicit declared check result without changing action evidence.
    pub(super) fn verify(&mut self, verification: Verification) {
        self.verification = verification;
    }

    /// Retain a separate check's cause and client facts without replacing the action account.
    pub(super) fn verification_failed(&mut self, failure: Self) {
        self.verification = Verification::Unavailable(match failure.conclusion {
            Conclusion::Refusal(cause) => cause,
            Conclusion::Outcome(_) => Refusal::IncompatibleReceipt,
        });
        self.decision = failure.decision;
        if let Some(reason) = failure.payload.facts.get("reason") {
            self.payload.facts["reason"] = reason.clone();
        }
        self.payload.facts["verification"] = failure.payload.facts;
    }

    /// Attach transient protocol-neutral client content outside retained truth.
    pub(super) fn with_content(mut self, content: ServiceContent) -> Self {
        self.payload.content.push(content);
        self
    }

    /// Set the typed meaning of successful recovery before resolution freezes.
    pub(super) fn replace_outcome(&mut self, outcome: Outcome) {
        self.conclusion = Conclusion::Outcome(outcome);
        self.repeat_candidate = false;
    }

    /// Consume the evidence once. Nothing after this point may change action truth.
    pub(super) fn resolve(
        self,
        execution: ExecutionEvidence,
        coverage: Option<Coverage>,
        observed: Observed,
    ) -> (Resolution, ClientPayload) {
        let observed = observed.merged(match &self.conclusion {
            Conclusion::Outcome(outcome) => outcome.observed(),
            Conclusion::Refusal(refusal) => refusal.observed(),
        });
        let preparation_only =
            execution.phase == Phase::Preparation && execution.progress.attempted == 0;
        let effect = if execution.uncertain
            || execution.pending_effects > 0
            || (self.declared_effect == Effect::Unknown && !preparation_only)
        {
            Effect::Unknown
        } else if self.declared_effect == Effect::Partial
            || (execution.progress.confirmed_effects > 0
                && execution
                    .progress
                    .expected
                    .is_some_and(|expected| execution.progress.confirmed_effects < expected))
        {
            Effect::Partial
        } else if self.declared_effect == Effect::Applied
            || execution.progress.confirmed_effects > 0
        {
            Effect::Applied
        } else {
            Effect::None
        };
        let status = if effect == Effect::Unknown {
            Status::Unknown
        } else {
            match &self.verification {
                Verification::NotMet => Status::Failed,
                Verification::Unavailable(Refusal::AuthorityBlocked { .. }) => Status::Blocked,
                Verification::Unavailable(_) => Status::Failed,
                _ if self.disposition == Status::Unknown => Status::Failed,
                _ => self.disposition,
            }
        };
        let repeat_safe = self.repeat_candidate
            && effect == Effect::None
            && matches!(
                self.verification,
                Verification::NotRequested | Verification::Met
            );
        let resolution = Resolution {
            invocation: self.invocation,
            status,
            effect,
            readiness: self.readiness,
            repeat_safe,
            conclusion: self.conclusion,
            verification: self.verification,
            phase: execution.phase,
            progress: execution.progress,
            decision: self.decision,
            physical_id: self.physical_id,
            coverage,
            observed,
            tools: self.tools,
            unconfirmed_history_steps: self.unconfirmed_history_steps,
        };
        (resolution, self.payload)
    }
}

/// Frozen product account. Caller, safe history and human views are projections of this value.
#[derive(Debug)]
pub(crate) struct Resolution {
    invocation: String,
    status: Status,
    effect: Effect,
    readiness: Readiness,
    repeat_safe: bool,
    conclusion: Conclusion,
    verification: Verification,
    phase: Phase,
    progress: Progress,
    decision: Decision,
    physical_id: Option<u64>,
    coverage: Option<Coverage>,
    observed: Observed,
    tools: Vec<String>,
    unconfirmed_history_steps: u32,
}

impl Resolution {
    /// Exercise projections through the same consuming resolver as production Work.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fixture(
        conclusion: Conclusion,
        status: Status,
        effect: Effect,
        verification: Verification,
        phase: Phase,
        progress: Progress,
        facts: Value,
    ) -> (Self, ClientPayload) {
        let mut evidence = WorkEvidence::new(
            "invocation_projection_fixture",
            status,
            effect,
            Readiness::Complete,
            effect == Effect::None,
            conclusion,
            facts,
            Decision::permitted(),
            None,
        );
        evidence.verify(verification);
        evidence.resolve(
            ExecutionEvidence {
                phase,
                progress,
                uncertain: effect == Effect::Unknown && phase == Phase::RequestedEffect,
                pending_effects: 0,
            },
            None,
            Observed::default(),
        )
    }
    /// Read the opaque invocation correlation identity.
    pub(crate) fn invocation(&self) -> &str {
        &self.invocation
    }
    /// Read the frozen terminal product disposition.
    pub(crate) const fn status(&self) -> Status {
        self.status
    }
    /// Read the frozen classification of the requested physical effects.
    pub(crate) const fn effect(&self) -> Effect {
        self.effect
    }
    /// Read the governed page readiness retained by the operation.
    pub(crate) const fn readiness(&self) -> Readiness {
        self.readiness
    }
    /// Read whether repeating the same request is known safe.
    pub(crate) const fn repeat_safe(&self) -> bool {
        self.repeat_safe
    }
    /// Read the typed operation meaning, independent of receipt wording.
    pub(crate) fn conclusion(&self) -> &Conclusion {
        &self.conclusion
    }
    /// Read the declared follow-up check's evidence.
    pub(crate) fn verification(&self) -> &Verification {
        &self.verification
    }
    /// Read the responsibility that supplied the final physical evidence.
    pub(crate) const fn phase(&self) -> Phase {
        self.phase
    }
    /// Read bounded confirmed physical progress, without client values.
    pub(crate) const fn progress(&self) -> Progress {
        self.progress
    }
    /// Read the deciding authority admission or refusal.
    pub(crate) const fn decision(&self) -> Decision {
        self.decision
    }
    /// Read the exact physical tab attributed to this operation.
    pub(crate) const fn physical_id(&self) -> Option<u64> {
        self.physical_id
    }
    /// Read the validated document coverage without embedded host names.
    pub(crate) fn coverage(&self) -> Option<&Coverage> {
        self.coverage.as_ref()
    }
    /// Read content-free browser and outcome measurements.
    pub(crate) fn observed(&self) -> &Observed {
        &self.observed
    }
    /// Read declared child tool identities before Language bounds their retention.
    pub(crate) fn tools(&self) -> &[String] {
        &self.tools
    }
    /// Read child storage failures independently of physical effects.
    pub(crate) const fn unconfirmed_history_steps(&self) -> u32 {
        self.unconfirmed_history_steps
    }

    /// Read the closed failure cause, preserving verification separately from action.
    pub(crate) fn cause(&self) -> Option<&Refusal> {
        match &self.verification {
            Verification::NotMet => Some(&Refusal::ExpectedConditionNotMet),
            Verification::Unavailable(cause) => Some(cause),
            _ => match &self.conclusion {
                Conclusion::Refusal(cause) => Some(cause),
                Conclusion::Outcome(_) => None,
            },
        }
    }
}

fn acknowledges_effect(outcome: &BrowserOutcome) -> bool {
    match outcome {
        BrowserOutcome::InDocuments { result, .. } => acknowledges_effect(result),
        BrowserOutcome::DialogAbsent { .. }
        | BrowserOutcome::RecordingAmbiguous { .. }
        | BrowserOutcome::RecordingNotFound
        | BrowserOutcome::RecordingExportFailed { .. } => false,
        BrowserOutcome::TabOpened { reused, .. } => !reused,
        BrowserOutcome::RecordingExported { delivery, .. } => !matches!(
            delivery,
            ghostlight_bridge::browser::RecordingDelivery::Returned { .. }
        ),
        _ => true,
    }
}

fn mutates(command: &BrowserCommand) -> bool {
    match command {
        BrowserCommand::InDocuments { primitive, .. } => mutates(primitive),
        BrowserCommand::DescribeDocuments { .. }
        | BrowserCommand::InstallPageRuntime { .. }
        | BrowserCommand::ListTabs
        | BrowserCommand::ReadText { .. }
        | BrowserCommand::Inspect { .. }
        | BrowserCommand::ReadDocument { .. }
        | BrowserCommand::InspectTree { .. }
        | BrowserCommand::Find { .. }
        | BrowserCommand::Screenshot { .. }
        | BrowserCommand::ScreenshotRegion { .. }
        | BrowserCommand::DescribeTargets { .. }
        | BrowserCommand::QuerySemantic { .. }
        | BrowserCommand::DescribeFocused { .. }
        | BrowserCommand::Observe { .. }
        | BrowserCommand::InspectDialog { .. }
        | BrowserCommand::ReadDiagnostics { .. }
        | BrowserCommand::StatusRecording { .. }
        | BrowserCommand::StartRecording { .. }
        | BrowserCommand::StopRecording { .. }
        | BrowserCommand::DiscardRecording { .. }
        | BrowserCommand::ClearDiagnostics { .. }
        | BrowserCommand::Present { .. } => false,
        BrowserCommand::FocusTab { .. }
        | BrowserCommand::OpenTab { .. }
        | BrowserCommand::Navigate { .. }
        | BrowserCommand::TraverseHistory { .. }
        | BrowserCommand::Reload { .. }
        | BrowserCommand::CloseTab { .. }
        | BrowserCommand::NavigateDiscardingBeforeUnload { .. }
        | BrowserCommand::Activate { .. }
        | BrowserCommand::ActivatePoint { .. }
        | BrowserCommand::ActivateModified { .. }
        | BrowserCommand::ActivatePointModified { .. }
        | BrowserCommand::WheelAt { .. }
        | BrowserCommand::Scroll { .. }
        | BrowserCommand::SetZoom { .. }
        | BrowserCommand::ResizeWindow { .. }
        | BrowserCommand::Hover { .. }
        | BrowserCommand::HoverPoint { .. }
        | BrowserCommand::Fill { .. }
        | BrowserCommand::TypeText { .. }
        | BrowserCommand::TypeFocused { .. }
        | BrowserCommand::PressKey { .. }
        | BrowserCommand::Drag { .. }
        | BrowserCommand::DragPoints { .. }
        | BrowserCommand::UploadFiles { .. }
        | BrowserCommand::DropImageAt { .. }
        | BrowserCommand::EvaluateScript { .. }
        | BrowserCommand::HandleDialog { .. }
        | BrowserCommand::ExportRecording { .. } => true,
        BrowserCommand::Cancel { .. } => false,
    }
}

fn compatible(command: &BrowserCommand, outcome: &BrowserOutcome) -> bool {
    use BrowserCommand as C;
    use BrowserOutcome as O;
    match (command, outcome) {
        (C::InDocuments { primitive, .. }, O::InDocuments { result, .. }) => {
            compatible(primitive, result)
        }
        (C::DescribeDocuments { .. }, O::Documents { .. })
        | (C::InstallPageRuntime { .. }, O::PageRuntimeInstalled { .. })
        | (C::ListTabs, O::Tabs { .. })
        | (C::FocusTab { .. }, O::TabFocused { .. })
        | (C::OpenTab { .. }, O::TabOpened { .. })
        | (
            C::Navigate { .. }
            | C::TraverseHistory { .. }
            | C::Reload { .. }
            | C::NavigateDiscardingBeforeUnload { .. },
            O::Navigated { .. },
        )
        | (C::CloseTab { .. }, O::TabClosed { .. })
        | (C::ReadText { .. } | C::ReadDocument { .. }, O::Text { .. })
        | (C::Inspect { .. } | C::Find { .. } | C::QuerySemantic { .. }, O::Targets { .. })
        | (C::InspectTree { .. }, O::DocumentTree { .. })
        | (C::Screenshot { .. } | C::ScreenshotRegion { .. }, O::Screenshot { .. })
        | (C::DescribeTargets { .. } | C::DescribeFocused { .. }, O::TargetsDescribed { .. })
        | (
            C::Activate { .. }
            | C::ActivatePoint { .. }
            | C::ActivateModified { .. }
            | C::ActivatePointModified { .. },
            O::Activated { .. },
        )
        | (C::WheelAt { .. } | C::Scroll { .. }, O::Scrolled { .. })
        | (C::SetZoom { .. }, O::Zoomed { .. })
        | (C::ResizeWindow { .. }, O::WindowResized { .. })
        | (C::Hover { .. } | C::HoverPoint { .. }, O::Hovered { .. })
        | (C::Fill { .. }, O::Filled { .. })
        | (C::TypeText { .. } | C::TypeFocused { .. }, O::Typed { .. })
        | (C::PressKey { .. }, O::KeyPressed { .. })
        | (C::Drag { .. } | C::DragPoints { .. }, O::Dragged { .. })
        | (C::UploadFiles { .. } | C::DropImageAt { .. }, O::FilesUploaded { .. })
        | (C::EvaluateScript { .. }, O::ScriptEvaluated { .. })
        | (C::Observe { .. }, O::Observed { .. })
        | (C::InspectDialog { .. }, O::Dialog { .. } | O::DialogAbsent { .. })
        | (C::HandleDialog { .. }, O::DialogHandled { .. } | O::DialogAbsent { .. })
        | (C::ReadDiagnostics { .. }, O::DiagnosticsRead { .. })
        | (C::ClearDiagnostics { .. }, O::DiagnosticsCleared { .. })
        | (C::StartRecording { .. }, O::RecordingStarted { .. })
        | (C::StatusRecording { .. }, O::RecordingStatus { .. })
        | (C::StopRecording { .. }, O::RecordingStopped { .. })
        | (
            C::ExportRecording { .. },
            O::RecordingExported { .. } | O::RecordingExportFailed { .. },
        )
        | (C::DiscardRecording { .. }, O::RecordingDiscarded { .. })
        | (C::Cancel { .. }, O::Cancelled)
        | (C::Present { .. }, O::Presented { .. }) => true,
        (
            C::StartRecording { .. }
            | C::StatusRecording { .. }
            | C::StopRecording { .. }
            | C::ExportRecording { .. }
            | C::DiscardRecording { .. },
            O::RecordingAmbiguous { .. } | O::RecordingNotFound,
        ) => true,
        _ => false,
    }
}

/// Validate the response's request identity after document-scope admission has completed.
pub(super) fn valid_receipt(command: &BrowserCommand, outcome: &BrowserOutcome) -> bool {
    if !compatible(command, outcome) {
        return false;
    }
    if let Some(expected) = command.tab_id() {
        if outcome_tab(outcome) != Some(expected) {
            return false;
        }
    }
    use BrowserCommand as C;
    use BrowserOutcome as O;
    match (command, outcome) {
        (C::InDocuments { primitive, .. }, O::InDocuments { result, .. }) => {
            valid_receipt(primitive, result)
        }
        (C::PressKey { key, .. }, O::KeyPressed { key: received, .. }) => key == received,
        (
            C::InstallPageRuntime {
                revision, sha256, ..
            },
            O::PageRuntimeInstalled {
                revision: received,
                sha256: hash,
            },
        ) => revision == received && sha256 == hash,
        (
            C::StatusRecording { recording_id }
            | C::StopRecording { recording_id }
            | C::ExportRecording { recording_id, .. },
            O::RecordingStatus { summary }
            | O::RecordingStopped { summary, .. }
            | O::RecordingExported { summary, .. },
        ) => {
            let identity = recording_id
                .as_ref()
                .is_none_or(|expected| expected == &summary.recording_id);
            identity
                && match (command, outcome) {
                    (
                        C::ExportRecording { destination, .. },
                        O::RecordingExported { delivery, .. },
                    ) => {
                        use ghostlight_bridge::browser::{
                            RecordingDelivery as R, RecordingDestination as D,
                        };
                        matches!(
                            (destination, delivery),
                            (D::Client, R::Returned { .. }) | (D::Download { .. }, R::Downloaded)
                        ) || matches!((destination, delivery), (D::Target { tab_id: expected, .. }, R::Attached { tab_id }) if expected == tab_id)
                    }
                    _ => true,
                }
        }
        (
            C::DiscardRecording {
                recording_id: Some(expected),
            },
            O::RecordingDiscarded { recording_id, .. },
        ) => expected == recording_id,
        _ => true,
    }
}

fn outcome_tab(outcome: &BrowserOutcome) -> Option<u64> {
    use BrowserOutcome as O;
    match outcome {
        O::InDocuments { result, .. } => outcome_tab(result),
        O::Documents { tab_id, .. }
        | O::TabFocused { tab_id, .. }
        | O::TabClosed { tab_id }
        | O::Text { tab_id, .. }
        | O::Targets { tab_id, .. }
        | O::DocumentTree { tab_id, .. }
        | O::Screenshot { tab_id, .. }
        | O::TargetsDescribed { tab_id, .. }
        | O::Scrolled { tab_id, .. }
        | O::Zoomed { tab_id, .. }
        | O::WindowResized { tab_id, .. }
        | O::Hovered { tab_id, .. }
        | O::FilesUploaded { tab_id, .. }
        | O::Observed { tab_id, .. }
        | O::Dialog { tab_id, .. }
        | O::DialogHandled { tab_id, .. }
        | O::DialogAbsent { tab_id }
        | O::DiagnosticsRead { tab_id, .. } => Some(*tab_id),
        O::TabOpened { tab, .. }
        | O::Navigated { tab, .. }
        | O::Activated { tab, .. }
        | O::Filled { tab, .. }
        | O::Typed { tab, .. }
        | O::KeyPressed { tab, .. }
        | O::Dragged { tab, .. }
        | O::ScriptEvaluated { tab, .. } => Some(tab.tab_id),
        O::RecordingStarted { summary, .. }
        | O::RecordingStatus { summary }
        | O::RecordingStopped { summary, .. }
        | O::RecordingExported { summary, .. } => Some(summary.tab_id),
        O::AttentionProtected { .. }
        | O::EffectUnknown { .. }
        | O::PageRuntimeInstalled { .. }
        | O::Tabs { .. }
        | O::DiagnosticsCleared { .. }
        | O::RecordingExportFailed { .. }
        | O::RecordingDiscarded { .. }
        | O::RecordingAmbiguous { .. }
        | O::RecordingNotFound
        | O::Presented { .. }
        | O::Cancelled => None,
    }
}
