//! One receipt/completion seam for direct operations, composition children, and preparation facts.

use super::*;
use crate::governance::evidence::{PermissionCheck, PermissionTrace};
use crate::language::history::StepReceipt;

impl ApplicationExecutor {
    /// Complete a direct operation or the composition parent through the shared receipt seam.
    pub(super) fn finish(
        &self,
        gate: &CompletionGate,
        terminal: Terminal,
        completion: Completion<'_>,
    ) -> InvocationResult {
        self.complete_terminal(gate, terminal, completion, None)
            .result
    }

    fn complete_terminal(
        &self,
        gate: &CompletionGate,
        mut terminal: Terminal,
        completion: Completion<'_>,
        step: Option<StepReceipt>,
    ) -> Terminal {
        let Completion {
            workspace,
            tool,
            requirements,
            snapshot,
            duration_ms,
            provenance,
        } = completion;
        if let Some(coverage) = self.take_coverage(terminal.result.invocation.as_str()) {
            terminal.result.summary =
                language::coverage::qualify(&terminal.result.summary, &coverage);
            terminal.audit = terminal.audit.with_coverage(&coverage);
            terminal.result.facts["coverage"] = json!(coverage);
        }
        let tool = language::audit::tool_name(tool);
        let event = match terminal.result.status {
            Status::Blocked
                if !matches!(
                    terminal.decision.reason,
                    ReasonCode::AuditUnavailable
                        | ReasonCode::RuntimeHold
                        | ReasonCode::SessionEnded
                ) && terminal.audit.refusal()
                    != Some(&crate::language::audit::AuditRefusal::CredentialAuthorization)
                    && (terminal.audit.composition().is_none() || !terminal.decision.allowed) =>
            {
                DomainEvent::WorkBlocked {
                    invocation: terminal.result.invocation.to_string(),
                    workspace: workspace.as_str().into(),
                    physical_id: terminal.physical_id,
                    presentation: denial_presentation(tool, &terminal.result),
                }
            }
            _ => DomainEvent::WorkCompleted {
                invocation: terminal.result.invocation.to_string(),
                workspace: workspace.as_str().into(),
                physical_id: terminal.physical_id,
            },
        };
        if step.is_none() {
            self.emit(event);
        } else {
            // A child finishes its own decoration without settling its parent's lifecycle
            // or showing a guardrail notice for every unsuccessful child in a composition.
            self.presentation.react(&DomainEvent::WorkCompleted {
                invocation: terminal.result.invocation.to_string(),
                workspace: workspace.as_str().into(),
                physical_id: terminal.physical_id,
            });
        }
        let status = terminal.result.status.stored_name();
        let effect = serde_json::to_value(terminal.result.effect)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        self.diagnostics.sink().emit(
            if terminal.result.status == Status::Failed {
                ghostlight_bridge::diagnostics::event::OPERATION_FAILED
            } else {
                ghostlight_bridge::diagnostics::event::OPERATION_COMPLETED
            },
            if terminal.result.status == Status::Failed {
                ghostlight_bridge::diagnostics::Level::Warn
            } else {
                ghostlight_bridge::diagnostics::Level::Info
            },
            Some(terminal.result.invocation.as_str()),
            &format!("{tool} {status} {effect} {}ms", duration_ms),
        );
        let observed = self
            .take_observation(terminal.result.invocation.as_str())
            .merged(terminal.observed.clone());
        // The record names a browser only when one received a command of this work.
        let browser_label = self.take_reached(terminal.result.invocation.as_str(), step.is_some());
        let mut record = AuditRecord::now(
            terminal.result.invocation.as_str(),
            workspace.as_str(),
            tool,
            requirements,
            snapshot.id(),
            terminal.decision,
            &status,
            &effect,
            &terminal.audit,
            duration_ms,
        )
        .with_provenance(Some(provenance.attribution()))
        .with_policy(snapshot, terminal.decision)
        .with_observation(observed)
        .with_browser(browser_label);
        record.step = step;
        record.permissions = self.take_permissions(terminal.result.invocation.as_str());
        let storage = self.audit.record_with_provenance(&record, Some(provenance));
        terminal.result.summary = if storage == language::audit_health::Storage::Saved {
            language::audit_health::qualify_children(
                &terminal.result.summary,
                terminal.audit.unconfirmed_history_steps(),
            )
        } else {
            storage.qualify(&terminal.result.summary)
        };
        terminal.result.history_storage = storage;
        gate.complete(terminal.result)
            .expect("single executor completion path");
        terminal.result = gate.take().expect("completion committed");
        terminal
    }

    /// Execute and complete an ordinary child without taking another lease or snapshot.
    pub(super) fn run_child(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        operation: &Operation,
        step: StepReceipt,
    ) -> Terminal {
        let _ = self.take_permissions(context.invocation.as_str());
        let started = Instant::now();
        let settlement_requested = language::settlement::enabled(operation);
        let settlement_pending = AtomicBool::new(settlement_requested);
        let context = InvocationContext {
            settlement_pending: &settlement_pending,
            settlement_requested,
            requirements: language::capability_map::requirements(operation),
            gesture: PageGesture::of(operation),
            ..*context
        };
        self.emit(DomainEvent::WorkPhaseStarted {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id: None,
            activity: operation_activity(operation),
        });
        let terminal = self.run(&context, lease, operation);
        self.complete_terminal(
            &CompletionGate::default(),
            terminal,
            Completion {
                workspace: context.workspace,
                tool: operation.name(),
                requirements: language::capability_map::requirements(operation),
                snapshot: context.snapshot,
                duration_ms: elapsed_ms(started),
                provenance: context.provenance,
            },
            Some(step),
        )
    }

    /// Record input preparation failure without fabricating a child invocation or browser effect.
    pub(super) fn record_preparation(
        &self,
        context: &InvocationContext<'_>,
        tool: &str,
        step: StepReceipt,
    ) -> language::audit_health::Storage {
        let mut record = AuditRecord::now(
            context.invocation.as_str(),
            context.workspace.as_str(),
            language::audit::tool_name(tool),
            CapabilitySet::EMPTY,
            context.snapshot.id(),
            Decision::refused(ReasonCode::InvalidRequest),
            "not_started",
            "none",
            &Outcome::StepNotStarted.audit(),
            0,
        )
        .with_provenance(Some(context.provenance.attribution()));
        record.step = Some(step);
        self.audit
            .record_with_provenance(&record, Some(context.provenance))
    }

    /// Retain the bounded evidence for the operation currently using this invocation.
    pub(super) fn retain_permission(
        &self,
        context: &InvocationContext<'_>,
        check: PermissionCheck,
    ) {
        self.permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(context.invocation.into())
            .or_default()
            .record(check);
    }

    fn take_permissions(&self, invocation: &str) -> PermissionTrace {
        self.permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(invocation)
            .unwrap_or_default()
    }
}
